use crate::capi::*;
use crate::capi as capi;
use std::sync::Arc;

#[derive(Clone)]
pub struct EndpointHandle {
    pointer: * mut reliable_endpoint_t,
}

impl Default for EndpointHandle {
    fn default() -> Self {
        Self { pointer: std::ptr::null_mut(), }
    }
}

impl EndpointHandle {
    pub fn new(config: &Config) -> Self {
        let pointer;
        unsafe {
            pointer = capi::reliable_endpoint_create(config, 100.0);
        }
        Self {
            pointer,
        }
    }

    pub fn set(&mut self, ptr: * mut reliable_endpoint_t) { self.pointer = ptr; }
    pub fn ptr(&self) -> * mut reliable_endpoint_t { self.pointer }
    pub fn ptr_mut(&mut self) -> * mut reliable_endpoint_t { self.pointer }
}

impl Into<* mut reliable_endpoint_t> for EndpointHandle {
    fn into(self) -> * mut reliable_endpoint_t {
        self.ptr()
    }
}

impl Drop for EndpointHandle {
    fn drop(&mut self) {
        trace!("EndpointHandle dropped, destroying handle");
        unsafe {
            reliable_endpoint_destroy(self.pointer);
        }
    }
}

pub trait Endpoint {
    fn handle(&self) -> Arc<EndpointHandle>;

    fn reset(&mut self) {
        trace!("reset");
        unsafe {
            capi::reliable_endpoint_reset(self.handle().ptr());
        }
    }

    fn update(&mut self, delta: f64) {
        trace!("update");
        unsafe {
            capi::reliable_endpoint_update(self.handle().ptr(), delta);
        }
    }

    fn clear_acks(&mut self) {
        trace!("clear_acks");
        unsafe {
            capi::reliable_endpoint_clear_acks(self.handle().ptr());
        }
    }

    fn next_packet_sequence(&self) -> u16 {
        unsafe {
            capi::reliable_endpoint_next_packet_sequence(self.handle().ptr())
        }
    }

    fn get_acks(&self) -> Vec<u16> {
        let mut num_acks: i32 = 0;
        let slice;

        unsafe {
            let ptr = capi::reliable_endpoint_get_acks(self.handle().ptr(), &mut num_acks);
            slice = std::slice::from_raw_parts(ptr, num_acks as usize);
        }

        slice.to_vec()
    }

    fn send(&mut self, packet: &[u8]) {
        unsafe {
            capi::reliable_endpoint_send_packet(self.handle().ptr(), packet.as_ptr(), packet.len() as i32);
        }
    }

    fn recv(&mut self, packet: &[u8]) {
        unsafe {
            capi::reliable_endpoint_receive_packet(self.handle().ptr(), packet.as_ptr(), packet.len() as i32);
        }
    }

    //
    //
    //

    fn current_rtt(&self) -> f32 {
        unsafe {
            capi::reliable_endpoint_rtt(self.handle().ptr())
        }
    }
    fn current_packet_loss(&self) -> f32 {
        unsafe {
            capi::reliable_endpoint_packet_loss(self.handle().ptr())
        }
    }

    fn bandwidth(&self) -> (f32, f32, f32) {
        let mut res: [f32; 3] = [0.0; 3];

        unsafe {
            capi::reliable_endpoint_bandwidth(self.handle().ptr(), &mut res[0], &mut res[1], &mut res[2])
        }

        (res[0], res[1], res[2])
    }
}



// Exposed function to the user of the bindings
pub fn create_packet_function<T, P>(config: &mut Config, transmit_packet: T, process_packet: P)
    where T: Fn(i32, u16, &[u8]),
          P: Fn(i32, u16, &[u8]) -> i32 {
    struct Context<T, P> {
        transmit_packet: T,
        process_packet: P,
    }
    let arg_context = Box::new(Context {
        transmit_packet: &transmit_packet,
        process_packet: &process_packet,
    });

    //ffi::do_thing(transmit_packet_function_wrapper::<F>, user_data);
    config.context = Box::into_raw(arg_context) as *mut std::os::raw::c_void;
    config.transmit_packet_function = Some(transmit_packet_wrapper::<T, P>);
    config.process_packet_function = Some(process_packet_wrapper::<P, T>);

    // Shim interface function
    extern fn transmit_packet_wrapper<T, U>(context : * mut std::os::raw::c_void ,
                                                  index : std::os::raw::c_int ,
                                                  sequence : u16 ,
                                                  buffer : * const u8 ,
                                                  size : std::os::raw::c_int)
        where T: Fn(i32, u16, &[u8]) {
        unsafe {
            let opt_context = Box::from_raw( context as *mut Context<T, U>);

            (opt_context.transmit_packet)(index as i32, sequence, std::slice::from_raw_parts(buffer, size as usize));
        }

    }

    extern fn process_packet_wrapper<P, U>(context : * mut std::os::raw::c_void ,
                                         index : std::os::raw::c_int ,
                                         sequence : u16 ,
                                         buffer : * const u8 ,
                                         size : std::os::raw::c_int) -> i32
        where P: Fn(i32, u16, &[u8]) -> i32 {
        unsafe {
            let opt_context = Box::from_raw( context as *mut Context<U, P>);
            (opt_context.process_packet)(index, sequence, std::slice::from_raw_parts(buffer, size as usize))
        }
    }
}

pub trait EndpointHandler {
    fn on_transmit_packet(&self, index: i32, sequence: u16, data: &[u8]);
    fn on_process_packet(&self, index: i32, sequence: u16, data: &[u8]) -> i32;
}

pub struct SimpleEndpoint<'a> {
    handle: Arc<EndpointHandle>,
    handler: Option<&'a dyn EndpointHandler>,
    config: Config,
}

impl<'a> SimpleEndpoint<'a> {
    pub fn new(mut config: Config, handler: &'a dyn EndpointHandler) -> Self {

        create_packet_function(&mut config,
                               |a, b, c| handler.on_transmit_packet(a, b, c),
                               |a, b, c| handler.on_process_packet(a, b, c),
        );

        Self {
            config,
            handler: Some(handler),
            handle: Arc::new(EndpointHandle::new(&config)),
        }
    }

    pub fn new_closure<T, P>(mut config: Config,  transmit_packet: T, process_packet: P) -> Self
        where T: Fn(i32, u16, &[u8]),
              P: Fn(i32, u16, &[u8]) -> i32
    {
        create_packet_function(&mut config,
                               transmit_packet,
                               process_packet,
        );

        Self {
            config,
            handler: None,
            handle: Arc::new(EndpointHandle::new(&config)),
        }
    }
}

impl<'a> Endpoint for SimpleEndpoint<'a> {
    fn handle(&self) -> Arc<EndpointHandle> { self.handle.clone() }
}

pub struct Reliable;
impl Reliable {
    pub fn new() -> Self {
        trace!("Reliable.io initialized");
        unsafe {
            capi::reliable_init();
        }
        Self {}
    }

    pub fn create_endpoint<'a>(&self, config: Config, handler: &'a dyn EndpointHandler, ) -> SimpleEndpoint<'a> {
        SimpleEndpoint::new(config, handler)
    }
    pub fn create_endpoint_closure<'a, T, P>(&self, config: Config, transmit_packet: T, process_packet: P) -> SimpleEndpoint<'a>
        where T: Fn(i32, u16, &[u8]),
              P: Fn(i32, u16, &[u8]) -> i32
    {
        SimpleEndpoint::new_closure(config, transmit_packet, process_packet)
    }
}

impl Drop for Reliable {
    fn drop(&mut self) {
        trace!("Reliable.io terminated");
        unsafe {
            capi::reliable_term();
        }
    }
}

pub use crate::capi::reliable_config_t as Config;

impl Default for Config {
    fn default() -> Self {
        trace!("Building config from default");
        unsafe {
            let mut res: Self = std::mem::uninitialized();
            capi::reliable_default_config(&mut res);

            res
        }
    }
}

mod tests {
    use super::*;

    struct TestHandler;
    impl EndpointHandler for TestHandler {
        fn on_transmit_packet(&self, index: i32, sequence: u16, data: &[u8]) {
            trace!("on_transmit_packet: {}. {}. {}", index, sequence, data.len());
        }

        fn on_process_packet(&self, index: i32, sequence: u16, data: &[u8]) -> i32 {
            trace!("on_process_packet: {}. {}. {}", index, sequence, data.len());

            data.len() as i32
        }
    }

    fn enable_logging() {
        use env_logger::Builder;
        use log::LevelFilter;

        Builder::new().filter(None, LevelFilter::Trace).init();

        unsafe {
            capi::reliable_log_level(RELIABLE_LOG_LEVEL_DEBUG as i32);
        }
    }

    #[test]
    fn reliable_init() {
        enable_logging();

        let reliable = Reliable::new();

        let handler = TestHandler{};
        let mut endpoint_1 = reliable.create_endpoint( Config::default(), &handler );
        let mut endpoint_2 = reliable.create_endpoint_closure( Config::default(),
                                                               |_, _, _| trace!("enter"),
                                                             |_, _, _| {trace!("enter"); 1}, );

        endpoint_1.update(10.0);
        endpoint_2.update(10.0);
    }

}