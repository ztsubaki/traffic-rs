use pyo3::{
    Py, PyAny, Python, exceptions::PyRuntimeError, pyclass, pymethods, types::PyAnyMethods,
};

use crate::{
    net::TrafficLightID,
    simulator::{
        PyNet,
        v2x::{Channel, Communicator, Message},
    },
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
        self.post_hook(time, v2x)
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
        self.post_hook(msg.time_arrive, v2x)
    }
    pub fn on_create(&mut self) -> Result<(), String> {
        if let Err(e) = Python::attach(|py| self.obj.bind(py).call_method0("on_create").map(|_| {}))
        {
            return Err(format!("Python Cloud on_create callback failed: {}", e));
        }
        Ok(())
    }
    fn post_hook(&mut self, time: usize, v2x: &mut Channel) -> Result<(), String> {
        let messages = Python::attach(|py| {
            let cloud_base =
                self.obj.bind(py).cast::<CloudBase>().map_err(|error| {
                    format!("Python Cloud must inherit from CloudBase: {error}")
                })?;
            Ok::<Vec<Message>, String>(cloud_base.borrow_mut().get_messages())
        })?;
        for message in messages {
            Message::new(v2x, message.content, message.from, message.to, time);
        }
        Ok(())
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

    fn on_tick(&self, _time: usize) -> pyo3::PyResult<()> {
        Err(PyRuntimeError::new_err("Cloud.on_tick must be overridden."))
    }

    fn on_message(
        &self,
        _time_send: usize,
        _time_receive: usize,
        _from: Option<TrafficLightID>,
        _content: Py<PyAny>,
    ) -> pyo3::PyResult<()> {
        Err(PyRuntimeError::new_err(
            "Cloud.on_message must be overridden.",
        ))
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

    fn on_create(&self) -> pyo3::PyResult<()> {
        Err(PyRuntimeError::new_err(
            "Cloud.on_create must be overridden.",
        ))
    }
}

impl CloudBase {
    pub fn get_messages(&mut self) -> Vec<Message> {
        std::mem::take(&mut self.messages)
    }
}
