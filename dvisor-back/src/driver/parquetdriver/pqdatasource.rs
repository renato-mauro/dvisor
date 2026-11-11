use std::rc::Rc;
use parquet::{column::reader::{ColumnReader, ColumnReaderImpl}, data_type::{ByteArray, ByteArrayType, DataType, Int64Type}, file::{metadata::{ColumnChunkMetaData, RowGroupMetaData}, reader::{FileReader, SerializedFileReader}}};
use crate::{driver::parquetdriver::urldataprovider::UrlChunkReader, err, error::DvisorResult, model::{castfrommacros::CastFrom, dvisordatasource::{DvisorType, IDvisorDataSource}, tiledata::TileData}};

pub struct ParquetDataSource {
    reader: SerializedFileReader<UrlChunkReader>
}

impl ParquetDataSource {

    pub fn new(url: &str) -> DvisorResult<Self> {
        let chunk_reader = UrlChunkReader::new(url)?;
        let reader = SerializedFileReader::new(chunk_reader)?;
        Ok(ParquetDataSource { reader })
    }

    fn get_row_group_meta(&self, row_group: usize) -> DvisorResult<&RowGroupMetaData> {
        let md = self.reader.metadata();
        if row_group < md.num_row_groups() {
            Ok(md.row_group(row_group))
        } else {
            err!("Row Group {} does not exist. Valid range: [0,{}).",row_group,self.get_row_group_count())
        }
    }

    fn get_column_meta(&self, row_group: usize, column: usize) -> DvisorResult<&ColumnChunkMetaData> {
        let rgm = self.get_row_group_meta(row_group)?;
        if column < rgm.num_columns() {
            Ok(rgm.column(column))
        } else {
            err!("Column {} does not exist in Row Group {}. Valid range: [0,{}).",column,row_group,rgm.num_columns())
        }
    }

    fn column_reader(&self, row_group: usize, column: usize) -> DvisorResult<ColumnReader> {
        let md = self.reader.metadata();
        let rgc = md.num_row_groups();
        if row_group >= rgc {
            err!("Row Group {} does not exist. Valid range: [0,{}).",row_group,rgc)
        }
        else {
            let cc = md.row_group(row_group).num_columns();
            if column >= cc {
                err!("Column {} does not exist in Row Group {}. Valid range: [0,{}).",column,row_group,cc)
            } else {
                Ok(self.reader.get_row_group(row_group)?.get_column_reader(column)?)
            }
        }
    }

}

fn read_vec<S,T>(mut cr: ColumnReaderImpl<S>, out: &mut Vec<T>) -> DvisorResult<()> 
    where
        S: DataType,
        S::T: Copy,
        T: CastFrom<S::T>
{
    let batch_size = 8192;
    let mut buf = Vec::with_capacity(batch_size);
    unsafe { buf.set_len(batch_size); }
    loop {
        let (total, _) = cr.read_batch(batch_size,None, None, buf.as_mut_slice())?;
        if total == 0 {
            break;
        }
        out.extend(buf[0..total].iter().map(|x|T::cast_from(*x)));
    }
    Ok(())
}

fn read_decimal(mut cr: ColumnReaderImpl<Int64Type>, out: &mut Vec<f32>, scale: i32) -> DvisorResult<()> 
{
    // Scaled in f64 and rounded once to f32, so each value is the f32 nearest
    // the decimal. An f32 factor is itself inexact (0.01 is stored about
    // 2e-8 low) and would shift every value, and every sum, the same way.
    let factor = 10f64.powi(-scale);
    let batch_size = 8192;
    let mut buf = Vec::with_capacity(batch_size);
    unsafe { buf.set_len(batch_size); }
    loop {
        let (total, _) = cr.read_batch(batch_size,None, None, buf.as_mut_slice())?;
        if total == 0 {
            break;
        }
        out.extend(buf[0..total].iter().map(|x|((*x as f64)*factor) as f32));
    }
    Ok(())
}

fn read_utf8(mut cr:ColumnReaderImpl<ByteArrayType>, out: &mut Vec<String>, mdl: i16) -> DvisorResult<()> {
    const BATCH: usize = 2048;
    let mut def_levels = vec![0i16; BATCH];
    let mut values: Vec<ByteArray> = vec![ByteArray::default(); BATCH];
    loop {
        let (values_read, levels_read) = cr.read_batch(BATCH, Some(&mut def_levels), None, &mut values)?;
        // A required column (max def level 0) has no definition levels:
        // read_batch reports levels_read == 0 and every value is present.
        if mdl == 0 {
            if values_read == 0 {
                break;
            }
            for v in &values[0..values_read] {
                out.push(str::from_utf8(v.data())?.to_owned());
            }
            continue;
        }
        if levels_read == 0 {
            break;
        }
        let mut v = 0usize;
        for i in 0..levels_read {
            if def_levels[i] == mdl {
                let bytes = values[v].data();
                out.push(str::from_utf8(bytes)?.to_owned());
                v += 1;
            } else { // null
                out.push(String::new());
            }
        }
    }
    Ok(())
}

impl IDvisorDataSource for ParquetDataSource {

    fn get_column_count(&self, row_group: usize) -> DvisorResult<usize> {
        if row_group < self.get_row_group_count() {
            Ok(self.reader.metadata().row_group(row_group).num_columns())
        } else {
            err!("Row Group {} does not exist. Valid range: [0,{}).",row_group,self.get_row_group_count())
        }
    }

    fn get_row_group_count(&self) -> usize {
        self.reader.metadata().num_row_groups()
    }

    fn get_tile(&self, row_group: usize, column: usize) -> DvisorResult<TileData> {
        let cr = self.column_reader(row_group, column)?;
        let cmeta = self.get_column_meta(row_group, column)?;
        let col_len = cmeta.num_values() as usize;
        let coldesc = cmeta.column_descr();
        let ctp = coldesc.converted_type();
        let mut is_date = false;
        let mut is_utf8 = false;
        let mut is_decimal = false;
        let mut scale = 0;
        match ctp {
            parquet::basic::ConvertedType::UTF8 => {
                is_utf8 = true;
            }
            parquet::basic::ConvertedType::DECIMAL => {
                is_decimal = true;
                scale = coldesc.type_scale();
            }
            parquet::basic::ConvertedType::DATE => {
                is_date = true;
            }
            _ => { }
        }
        match cr {
            ColumnReader::BoolColumnReader(_) => {
                err!("Not Implemented for type Bool")
            }
            ColumnReader::Int32ColumnReader(gcr) => {
                let mut out = Vec::with_capacity(col_len);
                read_vec(gcr, &mut out)?;
                if is_date {
                    Ok(TileData::Date(Rc::new(out)))
                } else {
                    Ok(TileData::I32(Rc::new(out)))
                }
            }
            ColumnReader::Int64ColumnReader(gcr) => {
                if is_decimal && scale > 0 {
                    let mut out = Vec::with_capacity(col_len);
                    read_decimal(gcr, &mut out, scale)?;
                    Ok(TileData::Decimal(Rc::new(out),scale))
                } else {
                    let mut out = Vec::with_capacity(col_len);
                    read_vec(gcr, &mut out)?;
                    Ok(TileData::I64(Rc::new(out)))
                }
            }
            ColumnReader::Int96ColumnReader(_) => {
                err!("Not Implemented for type Int96")
            }
            ColumnReader::FloatColumnReader(gcr) => {
                let mut out = Vec::with_capacity(col_len);
                read_vec(gcr, &mut out)?;
                Ok(TileData::Float(Rc::new(out)))
            }
            ColumnReader::DoubleColumnReader(gcr) => {
                let mut out = Vec::with_capacity(col_len);
                read_vec(gcr, &mut out)?;
                Ok(TileData::Double(Rc::new(out)))
            }
            ColumnReader::ByteArrayColumnReader(gcr) => {
                if is_utf8 {
                    let mdl = cmeta.column_descr().max_def_level();
                    let mut out = Vec::with_capacity(col_len);
                    read_utf8(gcr,&mut out,mdl)?;
                    Ok(TileData::String(Rc::new(out)))
                } else {
                    err!("Not Implemented for type ByteArray")
                }
            }
            ColumnReader::FixedLenByteArrayColumnReader(_) => {
                err!("Not Implemented for type FixedLenByte")
            }
        }
    }

    fn get_i8_tile(&self, row_group: usize, column: usize) -> DvisorResult<Vec<i8>> {
        let cr = self.column_reader(row_group, column)?;
        let mut out = Vec::with_capacity(self.get_num_rows_row_group(row_group)? as usize);
        match cr {
            ColumnReader::Int32ColumnReader(gcr) => {
                read_vec(gcr, &mut out)?;
                Ok(out)
            },
            ColumnReader::Int64ColumnReader(gcr) => {
                read_vec(gcr, &mut out)?;
                Ok(out)
            },
            _ => {
                err!("Column incompatible with i8")
            },
        }
    }

    fn get_i16_tile(&self, row_group: usize, column: usize) -> DvisorResult<Vec<i16>> {
        let cr = self.column_reader(row_group, column)?;
        let mut out = Vec::with_capacity(self.get_num_rows_row_group(row_group)? as usize);
        match cr {
            ColumnReader::Int32ColumnReader(gcr) => {
                read_vec(gcr, &mut out)?;
                Ok(out)
            },
            ColumnReader::Int64ColumnReader(gcr) => {
                read_vec(gcr, &mut out)?;
                Ok(out)
            },
            _ => {
                err!("Column incompatible with i16")
            },
        }
    }

    fn get_i32_tile(&self, row_group: usize, column: usize) -> DvisorResult<Vec<i32>> {
        let cr = self.column_reader(row_group, column)?;
        let mut out = Vec::with_capacity(self.get_num_rows_row_group(row_group)? as usize);
        match cr {
            ColumnReader::Int32ColumnReader(gcr) => {
                read_vec(gcr, &mut out)?;
                Ok(out)
            }
            ColumnReader::Int64ColumnReader(gcr) => {
                read_vec(gcr, &mut out)?;
                Ok(out)
            }
            _ => {
                err!("Column incompatible with i32")
            }
        }
    }

    fn get_i64_tile(&self, row_group: usize, column: usize) -> DvisorResult<Vec<i64>> {
        let cr = self.column_reader(row_group, column)?;
        let mut out = Vec::with_capacity(self.get_num_rows_row_group(row_group)? as usize);
        match cr {
            ColumnReader::Int32ColumnReader(gcr) => {
                read_vec(gcr, &mut out)?;
                Ok(out)
            }
            ColumnReader::Int64ColumnReader(gcr) => {
                read_vec(gcr, &mut out)?;
                Ok(out)
            }
            _ => {
                err!("Column incompatible with i64")
            }
        }
    }

    fn get_float_tile(&self, row_group: usize, column: usize) -> DvisorResult<Vec<f32>> {
        let cr = self.column_reader(row_group, column)?;
        let mut out = Vec::with_capacity(self.get_num_rows_row_group(row_group)? as usize);
        match cr {
            ColumnReader::FloatColumnReader(gcr) => {
                read_vec(gcr, &mut out)?;
                Ok(out)
            },
            ColumnReader::DoubleColumnReader(gcr) => {
                read_vec(gcr, &mut out)?;
                Ok(out)
            },
            _ => {
                err!("Column incompatible with f32")
            },
        }
    }

    fn get_decimal_tile(&self, row_group: usize, column: usize) -> DvisorResult<Vec<f32>> {
        let cr = self.column_reader(row_group, column)?;
        let cmeta = self.get_column_meta(row_group, column)?;
        let col_len = cmeta.num_values() as usize;
        let coldesc = cmeta.column_descr();
        let ctp = coldesc.converted_type();
        let mut is_decimal = false;
        let mut scale = 0;
        match ctp {
            parquet::basic::ConvertedType::DECIMAL => {
                is_decimal = true;
                scale = coldesc.type_scale();
            }
            _ => { }
        }
        match cr {
            ColumnReader::Int64ColumnReader(gcr) => {
                if is_decimal && scale > 0 {
                    let mut out = Vec::with_capacity(col_len);
                    read_decimal(gcr, &mut out, scale)?;
                    Ok(out)
                } else {
                    err!("Column incompatible with f32")
                }
            }
            _ => {
                err!("Column incompatible with f32")
            }
        }
    }

    fn get_double_tile(&self, row_group: usize, column: usize) -> DvisorResult<Vec<f64>> {
        let cr = self.column_reader(row_group, column)?;
        let mut out = Vec::with_capacity(self.get_num_rows_row_group(row_group)? as usize);
        match cr {
            ColumnReader::FloatColumnReader(gcr) => {
                read_vec(gcr, &mut out)?;
                Ok(out)
            },
            ColumnReader::DoubleColumnReader(gcr) => {
                read_vec(gcr, &mut out)?;
                Ok(out)
            },
            _ => {
                err!("Column incompatible with f64")
            },
        }
    }

    fn get_string_tile(&self, row_group: usize, column: usize) -> DvisorResult<Vec<String>> {
        let cr = self.column_reader(row_group, column)?;
        let cmeta = self.get_column_meta(row_group, column)?;
        let col_len = cmeta.num_values() as usize;
        match cr {
            ColumnReader::ByteArrayColumnReader(gcr) => {
                match cmeta.column_descr().converted_type() {
                    parquet::basic::ConvertedType::UTF8 => {
                        let mdl = cmeta.column_descr().max_def_level();
                        let mut out = Vec::with_capacity(col_len);
                        read_utf8(gcr,&mut out,mdl)?;
                        Ok(out)
                    },
                    _ => {
                        err!("Not Implemented for type ByteArray")
                    }
                }
            }
            _ => {
                err!("Column incompatible with String")
            }
        }
    }

    fn get_num_rows_row_group(&self, row_group: usize) -> DvisorResult<i64> {
        if row_group < self.get_row_group_count() {
            Ok(self.reader.metadata().row_group(row_group).num_rows())
        } else {
            err!("Row Group {} does not exist. Valid range: [0,{}).",row_group,self.get_row_group_count())
        }
    }
    
    fn get_num_rows(&self) -> i64 {
        self.reader.metadata().file_metadata().num_rows()
    }

    fn get_column_name(&self, row_group: usize, column: usize) -> DvisorResult<String> {
        let cmd = self.get_column_meta(row_group, column)?;
        Ok(cmd.column_descr().name().into())
    }
    
    fn get_column_internal_type(&self, row_group: usize, column: usize) -> DvisorResult<DvisorType> {
        let cmd = self.get_column_meta(row_group, column)?;
        let coldesc = cmd.column_descr();
        match cmd.column_descr().physical_type() {
            parquet::basic::Type::BOOLEAN => {
                err!("Not implemented for type Boolean")
            },
            parquet::basic::Type::INT32 => {
                Ok(DvisorType::I32)
            },
            parquet::basic::Type::INT64 => {
                Ok(DvisorType::I64)
            },
            parquet::basic::Type::INT96 => {
                err!("Not implemented for type Int96")
            },
            parquet::basic::Type::FLOAT => {
                Ok(DvisorType::Float)
            },
            parquet::basic::Type::DOUBLE => {
                Ok(DvisorType::Double)
            },
            parquet::basic::Type::BYTE_ARRAY => {
                let ctp = coldesc.converted_type();
                match ctp {
                    parquet::basic::ConvertedType::UTF8 => {
                        Ok(DvisorType::String)
                    }
                    _ => { 
                        err!("Not implemented for type ByteArray({:?})",ctp)
                    }
                }
            },
            parquet::basic::Type::FIXED_LEN_BYTE_ARRAY => {
                err!("Not implemented for type FixedLenByteArray")
            },
        }
    }
    
    fn get_column_byte_range(&self, row_group: usize, column: usize) -> DvisorResult<(u64,u64)> {
        let cmd = self.get_column_meta(row_group, column)?;
        Ok(cmd.byte_range())
    }
    
    fn get_column_index(&self, row_group: usize, column_name: &str) -> DvisorResult<usize> {
        let rgm = self.get_row_group_meta(row_group)?;
        let cn = column_name.to_lowercase();
        for (i,c) in rgm.columns().iter().enumerate() {
            let name = c.column_descr().name().to_lowercase();
            if name == cn {
                return Ok(i)
            }
        }
        let mut names = Vec::new();
        for c in rgm.columns().iter() {
            let name = c.column_descr().name().to_lowercase();
            names.push(name);
        }
        if let Some(s) = crate::utils::suggest(&names, &column_name.to_string()) {
            err!("Column {} not found in datasource. Did you mean '{}'?", column_name,s)
        } else {
            err!("Column {} not found in datasource.", column_name)
        }
    }
        
}



#[cfg(test)]
mod tests {
    use std::{fs::File, sync::Arc};
    use parquet::{
        column::reader::ColumnReader,
        data_type::{ByteArray, ByteArrayType},
        file::{properties::WriterProperties, reader::{FileReader, SerializedFileReader}, writer::SerializedFileWriter},
        schema::parser::parse_message_type,
    };
    use super::read_utf8;

    const N: usize = 5000; // spans several read_utf8 batches

    fn read_column(path: &std::path::Path, column: usize) -> Vec<String> {
        let reader = SerializedFileReader::new(File::open(path).unwrap()).unwrap();
        let rg = reader.get_row_group(0).unwrap();
        let mdl = rg.metadata().column(column).column_descr().max_def_level();
        let mut out = Vec::new();
        match rg.get_column_reader(column).unwrap() {
            ColumnReader::ByteArrayColumnReader(cr) => read_utf8(cr, &mut out, mdl).unwrap(),
            _ => panic!("expected a byte array column"),
        }
        out
    }

    #[test]
    fn read_utf8_reads_required_and_nullable_string_columns() {
        let schema = Arc::new(parse_message_type(
            "message s { REQUIRED BYTE_ARRAY req (UTF8); OPTIONAL BYTE_ARRAY opt (UTF8); }",
        ).unwrap());
        let path = std::env::temp_dir().join(format!("dvisor_read_utf8_{}.parquet", std::process::id()));

        let expected: Vec<String> = (0..N).map(|i| format!("v{}", i)).collect();
        let values: Vec<ByteArray> = expected.iter().map(|s| ByteArray::from(s.as_str())).collect();
        // Every third value of the nullable column is null.
        let def_levels: Vec<i16> = (0..N).map(|i| if i % 3 == 0 { 0 } else { 1 }).collect();
        let non_null: Vec<ByteArray> = (0..N).filter(|i| i % 3 != 0).map(|i| values[i].clone()).collect();

        let mut writer = SerializedFileWriter::new(
            File::create(&path).unwrap(), schema, Arc::new(WriterProperties::builder().build()),
        ).unwrap();
        let mut rg = writer.next_row_group().unwrap();
        let mut col = rg.next_column().unwrap().unwrap();
        col.typed::<ByteArrayType>().write_batch(&values, None, None).unwrap();
        col.close().unwrap();
        let mut col = rg.next_column().unwrap().unwrap();
        col.typed::<ByteArrayType>().write_batch(&non_null, Some(&def_levels), None).unwrap();
        col.close().unwrap();
        rg.close().unwrap();
        writer.close().unwrap();

        let required = read_column(&path, 0);
        let nullable = read_column(&path, 1);
        std::fs::remove_file(&path).ok();

        assert_eq!(required, expected);
        let expected_nullable: Vec<String> = (0..N)
            .map(|i| if i % 3 == 0 { String::new() } else { expected[i].clone() })
            .collect();
        assert_eq!(nullable, expected_nullable);
    }
}
