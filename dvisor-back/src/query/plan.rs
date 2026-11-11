use crate::{dvisor::Dvisor, error::DvisorResult, model::{datasourcewithmetadata::ITableFragmentProvider, enums::{AggFunction, SortOrder}, tablefragment::DvisorTableFragment, tiledata::{FloatOperator, IntOperator, StringOperator}}, query::ql::CalculatedExpression};

pub trait IQueryPipelineOperator {
    fn execute(&self, input: &DvisorTableFragment) -> DvisorResult<DvisorTableFragment>;
    fn check_feasible(&self, _provider: &Box<dyn ITableFragmentProvider>, _fragment_id: usize) -> DvisorResult<bool> {
        Ok(true)
    }
}

pub struct QueryPipeline {
    // `None` means "no projection was specified at all" (SELECT * — every
    // column is needed). `Some(v)` means exactly the named columns are
    // needed, and v CAN be empty (e.g. `SELECT count() FROM t` with no
    // WHERE needs zero columns) — that must not be confused with "all
    // columns", which is exactly the bug this distinction exists to avoid
    // (see get_table_fragment in datasourcewithmetadata.rs).
    columns: Option<Vec<String>>,
    operators: Vec<Box<dyn IQueryPipelineOperator>>
}

pub struct QueryResult {
    fragments: Vec<usize>,
    from: String,
    plan: QueryPipeline,
}

impl QueryResult {
    pub fn next(&mut self, dvisor: &mut Dvisor) -> DvisorResult<Option<DvisorTableFragment>> {
        if self.fragments.len() > 0 {
            let provider = dvisor.resolve(&self.from)?;
            let fragment_id = self.fragments.remove(0);
            Ok(Some(self.plan.execute_feasible_fragment(provider, fragment_id)?))
        } else {
            Ok(None)
        }
    }
}

impl QueryPipeline {
    pub fn new(columns: Option<Vec<String>>) -> QueryPipeline {
        QueryPipeline{ columns, operators: vec![] }
    }

    pub fn push_operator(&mut self, operator: Box<dyn IQueryPipelineOperator>) {
        self.operators.push(operator);
    }

    // fn execute_fragment(&self, provider: &Box<dyn ITableFragmentProvider>, fragment_id: usize) -> DvisorResult<Option<DvisorTableFragment>> {
    //     let mut is_feasible = true;
    //     for oper in &self.operators {
    //         if !oper.check_feasible(provider, fragment_id)? {
    //             is_feasible = false;
    //             break;
    //         }
    //     }
    //     if is_feasible {
    //         let mut t = provider.get_table_fragment(fragment_id, &self.columns)?;
    //         for oper in &self.operators {
    //             t = oper.execute(&t)?;
    //         }
    //         Ok(Some(t))
    //     } else {
    //         Ok(None)
    //     }
    // }

    pub fn execute_feasible_fragment(&self, provider: &Box<dyn ITableFragmentProvider>, fragment_id: usize) -> DvisorResult<DvisorTableFragment> {
        let mut t = provider.get_table_fragment(fragment_id, &self.columns)?;
        for oper in &self.operators {
            t = oper.execute(&t)?;
        }
        Ok(t)
    }

    pub fn execute(self, dvisor: &mut Dvisor, thread_id: usize, thread_count: usize, from: String) -> DvisorResult<QueryResult> {
        let provider = dvisor.resolve(&from)?;
        let fc = provider.get_fragment_count();
        let mut fragments = Vec::new();
        let mut fragment_id = thread_id;
        while fragment_id < fc {
            let mut is_feasible = true;
            for oper in &self.operators {
                if !oper.check_feasible(provider, fragment_id)? {
                    is_feasible = false;
                    break;
                }
            }
            if is_feasible {
                fragments.push(fragment_id);
            }
            fragment_id += thread_count;
        }
        Ok(QueryResult{fragments, from, plan: self})
    }
}

pub struct PipelineOperatorSelectInt {
    expr: String,
    oper: IntOperator
}

impl PipelineOperatorSelectInt {
    pub fn new(expr: String, oper: IntOperator) -> Box<dyn IQueryPipelineOperator> {
        Box::new(Self{expr,oper})
    }
}

impl IQueryPipelineOperator for PipelineOperatorSelectInt {
    fn execute(&self, input: &DvisorTableFragment) -> DvisorResult<DvisorTableFragment> {
        input.select_int(&self.expr, &self.oper)
    }
    
    fn check_feasible(&self, provider: &Box<dyn ITableFragmentProvider>, fragment_id: usize) -> DvisorResult<bool> {
        Ok(provider.check_feasible_int(fragment_id, &self.expr,&self.oper)?)
    }
}

pub struct PipelineOperatorGroupBy {
    // The group columns, in grouping order, by the names they are read by.
    group: Vec<(String,SortOrder)>,
    // The same columns' names in the output, which the merge compares by.
    group_output: Vec<String>,
    agfns: Vec<(String,AggFunction)>,
    alias: Vec<String>
}

impl PipelineOperatorGroupBy {
    pub fn new(group: Vec<(String,SortOrder)>, group_output: Vec<String>, agfns: Vec<(String,AggFunction)>, alias: Vec<String>) -> Box<dyn IQueryPipelineOperator> {
        Box::new(Self{group,group_output,agfns,alias})
    }
}

impl IQueryPipelineOperator for PipelineOperatorGroupBy {
    fn execute(&self, input: &DvisorTableFragment) -> DvisorResult<DvisorTableFragment> {
        let dt = input.sort(&self.group)?;
        dt.group_by(&self.group, &self.group_output, &self.agfns, &self.alias)
    }
}

pub struct PipelineOperatorSort {
    order: Vec<(String,SortOrder)>
}

impl PipelineOperatorSort {
    pub fn new(order: Vec<(String,SortOrder)>) -> Box<dyn IQueryPipelineOperator> {
        Box::new(Self{order})
    }
}

impl IQueryPipelineOperator for PipelineOperatorSort {
    fn execute(&self, input: &DvisorTableFragment) -> DvisorResult<DvisorTableFragment> {
        input.sort(&self.order)
    }
}

pub struct PipelineOperatorCalculateColumn {
    expression: CalculatedExpression
}

impl PipelineOperatorCalculateColumn {
    pub fn new(expression: CalculatedExpression) -> Box<dyn IQueryPipelineOperator> {
        Box::new(Self{ expression })
    }
}

impl IQueryPipelineOperator for PipelineOperatorCalculateColumn {
    fn execute(&self, input: &DvisorTableFragment) -> DvisorResult<DvisorTableFragment> {
        input.create_calculated_field(&self.expression)
    }
}

pub struct PipelineOperatorPack;

impl PipelineOperatorPack {
    pub fn new() -> Box<dyn IQueryPipelineOperator> {
        Box::new(Self{})
    }
}

impl IQueryPipelineOperator for PipelineOperatorPack {
    fn execute(&self, input: &DvisorTableFragment) -> DvisorResult<DvisorTableFragment> {
        input.pack_data()
    }
}

pub struct PipelineOperatorSelectFloat {
    expr: String,
    oper: FloatOperator
}

impl PipelineOperatorSelectFloat {
    pub fn new(expr: String, oper: FloatOperator) -> Box<dyn IQueryPipelineOperator> {
        Box::new(Self{expr,oper})
    }
}

impl IQueryPipelineOperator for PipelineOperatorSelectFloat {
    fn execute(&self, input: &DvisorTableFragment) -> DvisorResult<DvisorTableFragment> {
        input.select_float(&self.expr, &self.oper)
    }
    
    fn check_feasible(&self, provider: &Box<dyn ITableFragmentProvider>, fragment_id: usize) -> DvisorResult<bool> {
        Ok(provider.check_feasible_float(fragment_id, &self.expr,&self.oper)?)
    }
}

pub struct PipelineOperatorSelectString {
    expr: String,
    oper: StringOperator
}

impl PipelineOperatorSelectString {
    pub fn new(expr: String, oper: StringOperator) -> Box<dyn IQueryPipelineOperator> {
        Box::new(Self{expr,oper})
    }
}

impl IQueryPipelineOperator for PipelineOperatorSelectString {
    fn execute(&self, input: &DvisorTableFragment) -> DvisorResult<DvisorTableFragment> {
        input.select_string(&self.expr, &self.oper)
    }
    
    fn check_feasible(&self, provider: &Box<dyn ITableFragmentProvider>, fragment_id: usize) -> DvisorResult<bool> {
        Ok(provider.check_feasible_string(fragment_id, &self.expr,&self.oper)?)
    }
}

pub struct PipelineOperatorProject {
    columns: Vec<(String,String)>
}

impl PipelineOperatorProject {
    pub fn new(columns: Vec<(String,String)>) -> Box<dyn IQueryPipelineOperator> {
        Box::new(Self{columns})
    }
}

impl IQueryPipelineOperator for PipelineOperatorProject {
    fn execute(&self, input: &DvisorTableFragment) -> DvisorResult<DvisorTableFragment> {
        input.project(&self.columns)
    }
}




