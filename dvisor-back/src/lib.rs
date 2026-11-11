mod driver;
mod model;
mod query;
mod error;
mod requester;
mod urlrequest;
mod dvisor;
mod utils;
mod sql;

use js_sys::{Array, BigInt64Array, Float32Array, Float64Array, Int8Array, Int16Array, Int32Array, Object, Reflect};
use wasm_bindgen::prelude::*;
use crate::{dvisor::Dvisor, error::DvisorResult, model::{tablefragment::DvisorTableFragment, tiledata::TileData}, query::{helpercalccol::slice_to_array, plan::QueryResult}};

#[wasm_bindgen]
pub struct DvisorJsFacade {
    d: Dvisor,
    thread_id: usize,
    active_queries: Vec<(usize,QueryResult)>,
    current_index: usize
}

#[wasm_bindgen]
impl DvisorJsFacade {

    #[wasm_bindgen(constructor)]
    pub fn new(thread_id: usize, thread_count: usize) -> Self {
        console_error_panic_hook::set_once();
        let d = Dvisor::new(thread_id, thread_count);
        let active_queries = Vec::new();
        let current_index = 0;
        DvisorJsFacade { d, thread_id, active_queries, current_index }
    }

    #[wasm_bindgen]
    pub fn exec(&mut self, query: &str, qid: usize) -> Result<bool,JsValue> {
        self.active_queries.push((qid,self.d.exec(query)?)); 
        Ok(true)
    }

    #[wasm_bindgen]
    pub fn next(&mut self) -> Result<JsValue,JsValue> {
        let len = self.active_queries.len();
        if self.current_index < len {
            let thread_id = self.thread_id;
            let (qid,result) = &mut self.active_queries[self.current_index];
            let qid = *qid;
            let step = result.next(&mut self.d)
                .and_then(|otf| otf.map(|t| table_fragment_to_js(&t, qid, thread_id)).transpose());
            match step {
                Ok(Some(fragment)) => {
                    self.current_index += 1;
                    if self.current_index == len {
                        self.current_index = 0;
                    }
                    Ok(fragment)
                }
                Ok(None) => {
                    self.remove_query_at(self.current_index);
                    Ok(JsValue::from(qid))
                }
                Err(e) => {
                    // A failed query is over: drop it here rather than let
                    // the round-robin go on running its remaining fragments
                    // for a result nobody will read.
                    self.remove_query_at(self.current_index);
                    let obj = Object::new();
                    Reflect::set(&obj, &"source".into(), &e.into())?;
                    Reflect::set(&obj, &"qid".into(), &JsValue::from(qid))?;
                    Err(obj.into())
                }
            }
        } else {
            Ok(JsValue::FALSE)
        }
    }

    #[wasm_bindgen]
    pub fn cancel(&mut self, qid: usize) -> Result<JsValue,JsValue> {
        if let Some(index) = self.active_queries.iter().position(|(q,_)| *q == qid) {
            self.remove_query_at(index);
        }
        Ok(JsValue::TRUE)
    }

    #[wasm_bindgen]
    pub fn create_metadata(&self, file: &str) -> Result<JsValue,JsValue> {
        let md = self.d.create_metadata(file)?;
        Ok(JsValue::from(md.to_json()?))
    }

    #[wasm_bindgen(js_name = compileSql)]
    pub fn compile_sql(&self, sql: &str) -> Result<String,JsValue> {
        Ok(crate::sql::compile_sql(sql)?)
    }
}

impl DvisorJsFacade {
    // Removes a query from the round-robin, leaving `current_index` on the
    // query that would have run next — wrapping to the start when the
    // removed one was last. Left past the end, the index makes next()
    // answer `false`, the worker stops polling, and every other active query
    // in it stalls.
    fn remove_query_at(&mut self, index: usize) {
        self.active_queries.remove(index);
        if self.current_index > index {
            self.current_index -= 1;
        }
        if self.current_index >= self.active_queries.len() {
            self.current_index = 0;
        }
    }
}

fn table_fragment_to_js(tf: &DvisorTableFragment, qid: usize, thread_id: usize) -> DvisorResult<JsValue> {
    let obj = Array::new();
    let names = Array::new();
    for (name, td) in tf.get_columns() {
        let value: JsValue = match &**td {
            TileData::I8(items) => Int8Array::from(items.as_slice()).into(),
            TileData::I16(items) => Int16Array::from(items.as_slice()).into(),
            TileData::I32(items) => Int32Array::from(items.as_slice()).into(),
            TileData::I64(items) => BigInt64Array::from(items.as_slice()).into(),
            TileData::Float(items) => Float32Array::from(items.as_slice()).into(),
            TileData::Decimal(items,_) => Float32Array::from(items.as_slice()).into(),
            TileData::Double(items) => Float64Array::from(items.as_slice()).into(),
            TileData::String(items) => slice_to_array(items.as_slice()),
            TileData::Date(items) => Int32Array::from(items.as_slice()).into()
        };
        names.push(&name.into());
        obj.push(&value);
    }
    let helper = tf.js_helpers()?;
    Reflect::set(&obj, &"names".into(), &names).unwrap();
    Reflect::set(&obj, &"helper".into(), &helper.into()).unwrap();
    Reflect::set(&obj, &"qid".into(), &JsValue::from(qid)).unwrap();
    Reflect::set(&obj, &"thread_id".into(), &JsValue::from(thread_id)).unwrap();
    Ok(obj.into())
}

