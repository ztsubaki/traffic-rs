use pyo3::{Py, PyAny, Python, pyclass, pymethods, types::PyAnyMethods};

use crate::simulator::{
    PyNet,
    net::{Signals, TrafficLightID},
    v2x::{Channel, Communicator, Message},
};

pub struct Node {
    obj: Py<PyAny>,
}

impl Node {
    pub fn init(id: TrafficLightID, net: PyNet, class: &Py<PyAny>) -> Result<Self, String> {
        let obj = match Python::attach(|py| class.call1(py, (id, net))) {
            Ok(obj) => obj,
            Err(e) => return Err(format!("Failed to initialize Python Node: {}", e)),
        };
        Ok(Node { obj })
    }
    pub fn on_tick(&mut self, time: usize, v2x: &mut Channel) -> Result<Option<Signals>, String> {
        if let Err(e) = Python::attach(|py| {
            self.obj
                .bind(py)
                .call_method1("on_tick", (time,))
                .map(|_| {})
        }) {
            return Err(format!("Python Node on_tick callback failed: {}", e));
        }
        Ok(self.post_hook(time, v2x))
    }
    pub fn on_message(
        &mut self,
        msg: Message,
        v2x: &mut Channel,
    ) -> Result<Option<Signals>, String> {
        let from = match msg.from {
            Communicator::Cloud => None,
            Communicator::RSU(tls_id) => Some(tls_id),
            _ => {
                return Err(
                    "Node received a message from an unsupported sender; expected Cloud or RSU"
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
            return Err(format!("Python Node on_message callback failed: {}", e));
        }
        Ok(self.post_hook(msg.time_arrive, v2x))
    }
    pub fn on_create(&mut self) -> Result<(), String> {
        if let Err(e) = Python::attach(|py| self.obj.bind(py).call_method0("on_create").map(|_| {}))
        {
            return Err(format!("Python Node on_create callback failed: {}", e));
        }
        Ok(())
    }
    fn post_hook(&mut self, time: usize, v2x: &mut Channel) -> Option<Signals> {
        Python::attach(|py| {
            let mut obj = self.obj.bind(py).cast::<NodeBase>().unwrap().borrow_mut();
            for message in obj.get_messages() {
                Message::new(v2x, message.content, message.from, message.to, time);
            }
            obj.get_action()
        })
    }
}

#[pyclass(subclass)]
pub struct NodeBase {
    #[pyo3(get)]
    id: TrafficLightID,
    #[pyo3(get)]
    net: Py<PyNet>,
    messages: Vec<Message>,
    action: Option<Signals>,
}

#[pymethods]
impl NodeBase {
    #[new]
    fn new(id: TrafficLightID, net: Py<PyNet>) -> Self {
        NodeBase {
            id,
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
        _from: Option<TrafficLightID>,
        _content: Py<PyAny>,
    ) {
        panic!("on_message for Node not override");
    }

    fn on_create(&self) {
        panic!("on_create for Node not override");
    }

    fn send_to_cloud(&mut self, content: Py<PyAny>) {
        self.messages.push(Message {
            from: Communicator::Node(self.id),
            to: Communicator::Cloud,
            content,
            time_send: 0,
            time_arrive: 0,
        });
    }

    fn set_signal(&mut self, signals: String) {
        self.action = match Signals::from_str(&signals) {
            Ok(sig) => Some(sig),
            Err(e) => {
                log::warn!("Rejected invalid traffic signal state: {}", e);
                return;
            }
        };
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
