use pyo3::{Py, PyAny, Python, pyclass, pymethods, types::PyAnyMethods};

use crate::simluator::{
    PyNet,
    net::Signals,
    v2x::{Channel, Communicator, Message},
};

pub struct Node {
    obj: Py<PyAny>,
}

impl Node {
    pub fn new(obj: Py<PyAny>) -> Self {
        Node { obj }
    }
    pub fn on_tick(&mut self, time: usize, v2x: &mut Channel) {
        Python::attach(|py| {
            self.obj
                .bind(py)
                .call_method1("on_tick", (time,))
                .map(|_| {})
        })
        .unwrap();
        self.post_hook(time, v2x);
    }
    pub fn on_message(&mut self, msg: Message, v2x: &mut Channel) {
        let from = match msg.from {
            Communicator::Cloud => None,
            Communicator::RSU(tls_id) => Some(tls_id.0),
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
        })
        .unwrap();
        self.post_hook(msg.time_arrive, v2x);
    }
    pub fn on_create(&mut self, v2x: &mut Channel) {
        Python::attach(|py| self.obj.bind(py).call_method0("on_create").map(|_| {})).unwrap();
        self.post_hook(0, v2x);
    }
    fn post_hook(&mut self, time: usize, v2x: &mut Channel) {
        for message in Python::attach(|py| {
            self.obj
                .bind(py)
                .cast::<NodeBase>()
                .unwrap()
                .borrow_mut()
                .get_messages()
        }) {
            Message::new(v2x, message.content, message.from, message.to, time);
        }
    }
}

#[pyclass(subclass)]
pub struct NodeBase {
    #[pyo3(get)]
    node_id: usize,
    #[pyo3(get)]
    net: Py<PyNet>,
    messages: Vec<Message>,
    action: Option<Signals>,
}

#[pymethods]
impl NodeBase {
    #[new]
    fn new(id: usize, net: Py<PyNet>) -> Self {
        NodeBase {
            node_id: id,
            net,
            messages: Vec::new(),
            action: None,
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

    fn on_create(&self) {
        panic!("on_create for Node not override");
    }

    fn send_to_cloud(&mut self, content: Py<PyAny>) {
        self.messages.push(Message {
            from: Communicator::Node(super::net::TrafficLightID(self.node_id)),
            to: Communicator::Cloud,
            content,
            time_send: 0,
            time_arrive: 0,
        });
    }

    fn set_signal(&mut self, signals: String) {
        self.action = Signals::from_str(&signals).ok();
    }
}

impl NodeBase {
    pub fn get_messages(&mut self) -> Vec<Message> {
        std::mem::take(&mut self.messages)
    }
    pub fn get_action(&mut self) -> Option<Signals> {
        std::mem::take(&mut self.action)
    }
}
