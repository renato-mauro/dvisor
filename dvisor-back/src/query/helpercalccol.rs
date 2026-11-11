use std::{collections::HashMap, rc::Rc};
use js_sys::{Array, BigInt64Array, Float32Array, Float64Array, Function, Int16Array, Int32Array, Int8Array, Uint32Array};
use wasm_bindgen::JsValue;
use crate::{error::DvisorResult, model::{enums::CalcType, tiledata::TileData}, query::ql::CalculatedExpression, throw};

pub fn slice_to_array(strings: &[String]) -> JsValue {
    let arr = Array::new_with_length(strings.len() as u32);
    for (i, s) in strings.iter().enumerate() {
        arr.set(i as u32, JsValue::from(s.as_str()));
    }
    arr.into()
}

pub fn create_js_native_array_inplace(data: &TileData) -> JsValue {
    match data {
        TileData::I8(items) => {
            unsafe { Int8Array::view(&items[..]).into() }
        }
        TileData::I16(items) => {
            unsafe { Int16Array::view(&items[..]).into() }
        }
        TileData::I32(items) => {
            unsafe { Int32Array::view(&items[..]).into() }
        }
        TileData::I64(items) => {
            unsafe { BigInt64Array::view(&items[..]).into() }
        }
        TileData::Float(items) => {
            unsafe { Float32Array::view(&items[..]).into() }
        }
        TileData::Decimal(items,_) => {
            unsafe { Float32Array::view(&items[..]).into() }
        }
        TileData::Double(items) => {
            unsafe { Float64Array::view(&items[..]).into() }
        }
        TileData::String(items) => {
            slice_to_array(items.as_slice())
        }
        TileData::Date(items) => {
            unsafe { Int32Array::view(&items[..]).into() }
        }
    }
}

pub fn create_calc_col(expression: &CalculatedExpression, index: &Option<Rc<Vec<usize>>>, columns: &Vec<(String,Rc<TileData>)>) -> DvisorResult<TileData> {
    let len: usize = {
        if let Some((_,td)) = columns.first() {
            td.len()
        } else {
            0
        }
    };
    if len == 0 {
        throw!("Empty Table or Selection");
    }

    let mut name_pos = HashMap::new();
    let mut i: usize = 0;
    for (name,_) in columns {
        name_pos.insert(name,i);
        i += 1;
    }

    let mut test_call = String::with_capacity(128);
    let mut sep = "";
    for pn in &expression.get_param_names() {
        if let Some(pos) = name_pos.get(pn) {
            let td = &columns[*pos].1;
            let testc = td.js_helper_test();
            test_call.push_str(format!("{}{}",sep,testc).as_str());
            sep = ","
        } else {
            throw!("Column name {} not found in expression {}",pn,expression.expr);
        }
    }
    let ctp = if let Some(ectp) = &expression.tp {
        *ectp
    } else {
        let ft = Function::new_no_args(format!("const api=this.API;const f=({});let r=f({});if(r instanceof Date)return 'DATE';else if(typeof r==='number' && Number.isFinite(r))if(Number.isInteger(r))return 'INT';else return 'FLOAT';else return 'STRING';",expression.expr,test_call).as_str());
        if let Some(value) = ft.call0(&JsValue::null())?.as_string() {
            match value.as_str() {
                "DATE" => CalcType::DATE,
                "INT" => CalcType::INT,
                "FLOAT" => CalcType::FLOAT,
                _ => CalcType::STRING
            }
        } else {
            CalcType::STRING
        }
    };

    let mut table_call = String::with_capacity(128);
    sep = "";
    for pn in &expression.get_param_names() {
        if let Some(pos) = name_pos.get(pn) {
            let td = &columns[*pos].1;
            let tc = td.js_helper_calc(format!("t[{}][i]",pos));
            table_call.push_str(format!("{}{}",sep,tc).as_str());
            sep = ","
        } else {
            throw!("Column name {} not found in expression {}",pn,expression.expr);
        }
    }
    let f = match ctp {
        CalcType::DATE => {
            format!("~~(f({table_call})/86400000)")
        }
        _ => {
            format!("f({table_call})")
        }
    };

    // Everything above this point only builds strings and does ordinary
    // Rust allocations. Nothing below it may allocate: js_sys::TypedArray
    // ::view() below returns a *zero-copy* view straight into wasm linear
    // memory, and the moment anything triggers `memory.grow()` (even an
    // unrelated Vec/String reallocation), the JS engine detaches the old
    // ArrayBuffer those views point into — silently, with no error. Writes
    // through a detached view are simply dropped, leaving the destination
    // Vec's uninitialized bytes in place (see the calc-column write-up).
    // So: finish the code string, compile the Function, and only then
    // create the views, with the JS call immediately after.
    let mut code = String::with_capacity(1024);
    let js_fn = if index.is_some() {
        code.push_str(format!("const api=this.API;const f=({});for(let j=0;j<idx.length;j++){{",expression.expr).as_str());
        code.push_str(format!("let i=idx[j];o[i]={}}}",f).as_str());
        Function::new_with_args("idx,t,o",&code)
    } else {
        code.push_str(format!("const api=this.API;const f=({});for(let i=0;i<t[0].length;i++){{",expression.expr).as_str());
        code.push_str(format!("o[i]={}}}",f).as_str());
        Function::new_with_args("t,o",&code)
    };

    let out = match ctp {
        CalcType::INT => {
            let mut dt = Vec::with_capacity(len);
            unsafe { dt.set_len(len); }
            Some(TileData::I32(Rc::new(dt)))
        }
        CalcType::FLOAT => {
            let mut dt = Vec::with_capacity(len);
            unsafe { dt.set_len(len); }
            Some(TileData::Float(Rc::new(dt)))
        }
        CalcType::DATE => {
            let mut dt = Vec::with_capacity(len);
            unsafe { dt.set_len(len); }
            Some(TileData::Date(Rc::new(dt)))
        }
        CalcType::STRING => {
            None
        }
    };
    let js_out: JsValue = match ctp {
        CalcType::STRING => {
            JsValue::from(Array::new())
        }
        _ => {
            create_js_native_array_inplace(out.as_ref().unwrap())
        }
    };
    let js_table = Array::new();
    for (_,td) in columns {
        js_table.push(&create_js_native_array_inplace(&td));
    }

    if let Some(idx) = index {
        let js_idx:JsValue = unsafe {
            let slice = std::slice::from_raw_parts(idx.as_ptr() as *const u32, idx.len());
            Uint32Array::view(slice).into()
        };
        js_fn.call3(&JsValue::null(),&js_idx,&js_table,&js_out)?;
    } else {
        js_fn.call2(&JsValue::null(),&js_table,&js_out)?;
    }

    match ctp {
        CalcType::STRING => {
            let a = Array::from(&js_out);
            let data: Vec<String> = a.iter()
                .map(|v| v.as_string().unwrap_or_default())
                .collect()
            ;
            Ok(TileData::String(Rc::new(data)))
        }
        _ => {
            Ok(out.unwrap())
        }
    }
}
