#![no_std]
#![no_main]

use selfe_runtime as _;

use console::ProcParams;
use core::fmt::{self, Write as WriteFmt};
use ferros::{cap::role, userland::Caller};
use menu::*;
use sabrelite_bsp::debug_logger::DebugLogger;
use sabrelite_bsp::embedded_hal::serial::Read;
use sabrelite_bsp::imx6_hal::pac::uart1::UART1;
use sabrelite_bsp::imx6_hal::serial::Serial;

static LOGGER: DebugLogger = DebugLogger;

#[allow(improper_ctypes_definitions)]
#[no_mangle]
pub extern "C" fn _start(params: ProcParams<role::Local>) -> ! {
    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(DebugLogger::max_log_level_from_env()))
        .unwrap();

    log::debug!("[console] process started");

    let int_consumer = params.int_consumer;
    let serial = Serial::new(params.uart);
    let context = Context {
        serial,
        storage_caller: params.storage_caller,
    };

    // Console buffer on the stack
    let mut buffer = [0_u8; 128];
    let state = Runner::new(&ROOT_MENU, &mut buffer, context);

    log::info!("[console] run 'telnet 0.0.0.0 8888' to connect to the console interface");
    int_consumer.consume(state, move |mut state| {
        if let Ok(b) = state.context.serial.read() {
            state.input_byte(b);
        }
        state
    })
}

pub struct Context {
    serial: Serial<UART1>,
    storage_caller: Caller<
        persistent_storage::Request,
        Result<persistent_storage::Response, persistent_storage::ErrorCode>,
        role::Local,
    >,
}

impl fmt::Write for Context {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.serial.write_str(s)
    }
}

const ROOT_MENU: Menu<Context> = Menu {
    label: "root",
    items: &[&Item {
        command: "storage",
        help: Some("Enter the persistent storage sub-menu."),
        item_type: ItemType::Menu(&Menu {
            label: "storage",
            items: &[
                &Item {
                    command: "append",
                    help: Some(storage::append::HELP),
                    item_type: ItemType::Callback {
                        function: storage::append::cmd,
                        parameters: &[
                            Parameter::Mandatory {
                                parameter_name: "key",
                                help: Some("The entry's key string"),
                            },
                            Parameter::Mandatory {
                                parameter_name: "value",
                                help: Some("The entry's value string"),
                            },
                        ],
                    },
                },
                &Item {
                    command: "get",
                    help: Some(storage::get::HELP),
                    item_type: ItemType::Callback {
                        function: storage::get::cmd,
                        parameters: &[Parameter::Mandatory {
                            parameter_name: "key",
                            help: Some("The entry's key string"),
                        }],
                    },
                },
                &Item {
                    command: "invalidate",
                    help: Some(storage::invalidate::HELP),
                    item_type: ItemType::Callback {
                        function: storage::invalidate::cmd,
                        parameters: &[Parameter::Mandatory {
                            parameter_name: "key",
                            help: Some("The entry's key string"),
                        }],
                    },
                },
                &Item {
                    command: "gc",
                    help: Some(storage::gc::HELP),
                    item_type: ItemType::Callback {
                        function: storage::gc::cmd,
                        parameters: &[],
                    },
                },
            ],
            entry: None,
            exit: None,
        }),
    }],
    entry: Some(enter_root_menu),
    exit: None,
};

// NOTE: you won't see this in QEMU emulation unless you remove
// the 'nowait' parameter from the QEMU invocation
// in scripts/simulate.sh
fn enter_root_menu(_menu: &Menu<Context>, context: &mut Context) {
    writeln!(context, "\n\n").unwrap();
    writeln!(context, "***************************").unwrap();
    writeln!(context, "* Welcome to the console! *").unwrap();
    writeln!(context, "***************************").unwrap();
}

mod storage {
    use super::*;
    use persistent_storage::{ErrorCode, Key, Request, Response, Value};

    fn print_resp(context: &mut Context, resp: &Result<Response, ErrorCode>) {
        if let Ok(r) = resp {
            writeln!(context.serial, "{}", r).unwrap();
        } else {
            writeln!(context.serial, "{:?}", resp).unwrap();
        }
    }

    pub mod append {
        use super::*;

        pub const HELP: &str = "Appends the key/value pair to storage.

  Example:
  append my-key my-data";

        pub fn cmd(
            _menu: &Menu<Context>,
            item: &Item<Context>,
            args: &[&str],
            context: &mut Context,
        ) {
            let key = Key::from(menu::argument_finder(item, args, "key").unwrap().unwrap());
            let value = Value::from(menu::argument_finder(item, args, "value").unwrap().unwrap());

            log::debug!(
                "[console] Append storage item key='{}' value='{}'",
                key,
                value
            );

            let resp = context
                .storage_caller
                .blocking_call(&Request::AppendKey(key, value))
                .expect("Failed to perform a blocking_call");

            print_resp(context, &resp);
        }
    }

    pub mod get {
        use super::*;

        pub const HELP: &str = "Retrieves the value for the given key from storage.

  Example:
  get my-key";

        pub fn cmd(
            _menu: &Menu<Context>,
            item: &Item<Context>,
            args: &[&str],
            context: &mut Context,
        ) {
            let key = Key::from(menu::argument_finder(item, args, "key").unwrap().unwrap());

            log::debug!("[console] Get storage value for key='{}'", key);

            let resp = context
                .storage_caller
                .blocking_call(&Request::Get(key))
                .expect("Failed to perform a blocking_call");

            print_resp(context, &resp);
        }
    }

    pub mod invalidate {
        use super::*;

        pub const HELP: &str = "Invalidates the key in storage.

  Example:
  invalidate my-key";

        pub fn cmd(
            _menu: &Menu<Context>,
            item: &Item<Context>,
            args: &[&str],
            context: &mut Context,
        ) {
            let key = Key::from(menu::argument_finder(item, args, "key").unwrap().unwrap());

            log::debug!("[console] Invalidate storage key='{}'", key);

            let resp = context
                .storage_caller
                .blocking_call(&Request::InvalidateKey(key))
                .expect("Failed to perform a blocking_call");

            print_resp(context, &resp);
        }
    }

    pub mod gc {
        use super::*;

        pub const HELP: &str = "Perform a garbage collection on storage.

  Example:
  gc";

        pub fn cmd(
            _menu: &Menu<Context>,
            _item: &Item<Context>,
            _args: &[&str],
            context: &mut Context,
        ) {
            log::debug!("[console] Garbage collect storage");

            let resp = context
                .storage_caller
                .blocking_call(&Request::GarbageCollect)
                .expect("Failed to perform a blocking_call");

            print_resp(context, &resp);
        }
    }
}
