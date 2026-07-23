use std::cell::RefCell;
use std::collections::HashMap;

// 线程本地缓存：Arc 指针 → Python 对象
thread_local! {
    static IDENTITY_CACHE: RefCell<HashMap<usize, pyo3::PyObject>> = RefCell::new(HashMap::new());
}

/// 从缓存获取 Python 对象。命中返回 Some，否则返回 None。
pub fn cache_get(key: usize) -> Option<pyo3::PyObject> {
    IDENTITY_CACHE.with(|cache| {
        cache
            .borrow()
            .get(&key)
            .map(|obj| pyo3::Python::with_gil(|py| obj.clone_ref(py)))
    })
}

/// 将 Python 对象写入缓存。
pub fn cache_insert(key: usize, obj: &pyo3::PyObject) {
    IDENTITY_CACHE.with(|cache| {
        let owned = pyo3::Python::with_gil(|py| obj.clone_ref(py));
        cache.borrow_mut().insert(key, owned);
    });
}

/// 清理已被 Python GC 回收的条目（引用计数为 1 说明只有缓存持有）。
pub fn cache_gc() {
    IDENTITY_CACHE.with(|cache| {
        cache
            .borrow_mut()
            .retain(|_, obj| pyo3::Python::with_gil(|py| obj.get_refcnt(py) > 1));
    });
}
