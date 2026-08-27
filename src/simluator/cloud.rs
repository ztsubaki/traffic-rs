use pyo3::{Py, PyAny, Python, pyclass, pymethods, types::PyAnyMethods};

use crate::simluator::v2x::Message;

pub struct Cloud {
    obj: Py<PyAny>,
}

impl Cloud {
    pub fn new(obj: Py<PyAny>) -> Self {
        let mut cloud = Cloud { obj };
        cloud.on_create();
        cloud
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
pub struct CloudBase {}

#[pymethods]
impl CloudBase {
    #[new]
    fn new() -> Self {
        CloudBase {}
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
