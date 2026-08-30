use pyo3::{Py, PyAny, Python, pyclass, pymethods, types::PyAnyMethods};

use crate::simulator::{
    PyNet,
    net::TrafficLightID,
    v2x::{Channel, Communicator, Message},
};

pub struct Cloud {
    obj: Py<PyAny>,
}

impl Cloud {
    pub fn init(net: PyNet, class: &Py<PyAny>) -> Result<Self, String> {
        let obj = match Python::attach(|py| class.call1(py, (net,))) {
            Ok(obj) => obj,
            Err(e) => return Err(format!("Failed to initialize Python Cloud: {}", e)),
        };
        Ok(Cloud { obj })
    }
    pub fn on_tick(&mut self, time: usize, v2x: &mut Channel) -> Result<(), String> {
        if let Err(e) = Python::attach(|py| {
            self.obj
                .bind(py)
                .call_method1("on_tick", (time,))
                .map(|_| {})
        }) {
            return Err(format!("Python Cloud on_tick callback failed: {}", e));
        }
        self.post_hook(time, v2x);
        Ok(())
    }
    pub fn on_message(&mut self, msg: Message, v2x: &mut Channel) -> Result<(), String> {
        let from = match msg.from {
            Communicator::Node(tls_id) => Some(tls_id.to_usize()),
            _ => {
                return Err(
                    "Cloud received a message from an unsupported sender; expected Node"
                        .to_string(),
                );
            }
        };
        if let Err(e) = Python::attach(|py| {
            self.obj
                .bind(py)
                .call_method1(
                    "on_message",
                    (msg.time_send, msg.time_arrive, from, msg.content),
                )
                .map(|_| {})
        }) {
            return Err(format!("Python Cloud on_message callback failed: {}", e));
        }
        self.post_hook(msg.time_arrive, v2x);
        Ok(())
    }
    pub fn on_create(&mut self) -> Result<(), String> {
        if let Err(e) = Python::attach(|py| self.obj.bind(py).call_method0("on_create").map(|_| {}))
        {
            return Err(format!("Python Cloud on_create callback failed: {}", e));
        }
        Ok(())
    }
    fn post_hook(&mut self, time: usize, v2x: &mut Channel) {
        for message in Python::attach(|py| {
            self.obj
                .bind(py)
                .cast::<CloudBase>()
                .unwrap()
                .borrow_mut()
                .get_messages()
        }) {
            Message::new(v2x, message.content, message.from, message.to, time);
        }
    }
}

#[pyclass(subclass)]
pub struct CloudBase {
    #[pyo3(get)]
    net: Py<PyNet>,
    messages: Vec<Message>,
}

#[pymethods]
impl CloudBase {
    #[new]
    fn new(net: Py<PyNet>) -> Self {
        CloudBase {
            messages: Vec::new(),
            net,
        }
    }

    fn on_tick(&self, _time: usize) {
        panic!("on_tick for Node not override");
    }

    fn on_message(
        &self,
        _time_send: usize,
        _time_receive: usize,
        _from: Option<TrafficLightID>,
        _content: Py<PyAny>,
    ) {
        panic!("on_message for Node not override");
    }

    fn send_to_node(&mut self, node_id: TrafficLightID, content: Py<PyAny>) {
        self.messages.push(Message {
            from: Communicator::Cloud,
            to: Communicator::Node(node_id),
            content,
            time_send: 0,
            time_arrive: 0,
        });
    }

    fn on_create(&self) {
        panic!("on_create for Node not override");
    }
}

impl CloudBase {
    pub fn get_messages(&mut self) -> Vec<Message> {
        std::mem::take(&mut self.messages)
    }
}
