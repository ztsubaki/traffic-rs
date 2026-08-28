use pyo3::{Py, PyAny, Python, pyclass, pymethods, types::PyAnyMethods};

use crate::simluator::{
    net::{Net, Signal},
    v2x::{Channel, Communicator, Message},
};

pub struct Cloud {
    obj: Py<PyAny>,
}

impl Cloud {
    pub fn new(obj: Py<PyAny>) -> Self {
        Cloud { obj }
    }
    pub fn on_tick(&mut self, time: usize, v2x: &mut Channel, net: &Net) {
        Python::attach(|py| {
            self.obj
                .bind(py)
                .call_method1("on_tick", (time,))
                .map(|_| {})
        })
        .unwrap();
        self.post_hook(time, v2x);
    }
    pub fn on_message(&mut self, msg: Message, v2x: &mut Channel, net: &Net) {
        let from = match msg.from {
            Communicator::Node(tls_id) => Some(tls_id.0),
            _ => {
                return;
            }
        };
        Python::attach(|py| {
            self.obj
                .bind(py)
                .call_method1(
                    "on_message",
                    (msg.time_send, msg.time_arrive, from, msg.content),
                )
                .map(|_| {})
        });
        self.post_hook(msg.time_arrive, v2x);
    }
    pub fn on_create(&mut self, v2x: &mut Channel, net: &Net) {
        Python::attach(|py| self.obj.bind(py).call_method0("on_create").map(|_| {})).unwrap();
        self.post_hook(0, v2x);
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
    messages: Vec<Message>,
}

#[pymethods]
impl CloudBase {
    #[new]
    fn new() -> Self {
        CloudBase {
            messages: Vec::new(),
        }
    }

    fn on_tick(&self, _time: usize) {
        panic!("on_tick for Node not override");
    }

    fn on_message(
        &self,
        _time_send: usize,
        _time_receive: usize,
        _from: Option<usize>,
        _content: Py<PyAny>,
    ) {
        panic!("on_message for Node not override");
    }

    fn send_to_node(&mut self, node_id: usize, content: Py<PyAny>) {
        self.messages.push(Message {
            from: Communicator::Cloud,
            to: Communicator::Node(super::net::TrafficLightID(node_id)),
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
