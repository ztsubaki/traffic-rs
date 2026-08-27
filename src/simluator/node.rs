use pyo3::{Py, PyAny, Python, pyclass, pymethods, types::PyAnyMethods};

use crate::simluator::v2x::Message;

pub struct Node {
    obj: Py<PyAny>,
}

impl Node {
    pub fn new(obj: Py<PyAny>) -> Self {
        let mut node = Node { obj };
        node.on_create();
        node
    }
    pub fn on_tick(&mut self, time: usize) {
        Python::attach(|py| {
            self.obj
                .bind(py)
                .call_method1("on_tick", (time,))
                .map(|_| {})
        })
        .unwrap();
    }
    pub fn on_message(&mut self, msg: Message) {}
    pub fn on_create(&mut self) {
        Python::attach(|py| self.obj.bind(py).call_method0("on_create").map(|_| {})).unwrap();
    }
}

#[pyclass(subclass)]
pub struct NodeBase {
    #[pyo3(get)]
    node_id: usize,
}

#[pymethods]
impl NodeBase {
    #[new]
    fn new(id: usize) -> Self {
        NodeBase { node_id: id }
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
}
