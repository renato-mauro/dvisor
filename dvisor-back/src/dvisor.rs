use std::{collections::HashMap, rc::Rc};
use crate::{error::DvisorResult, model::{datasourcewithmetadata::ITableFragmentProvider, metadata::Metadata}, query::{plan::QueryResult, ql::Query}, requester::Requester};

pub struct Dvisor {
    thread_id: usize,
    thread_count: usize,
    requester: Requester,
    providers: HashMap<String,Rc<Box<dyn ITableFragmentProvider>>>
}

impl Dvisor {

    pub fn new(thread_id: usize, thread_count: usize) -> Dvisor {
        let requester = Requester::new();
        let providers = HashMap::new();
        Dvisor { thread_id, thread_count, requester, providers }
    }

    pub fn resolve(&mut self, from: &str) -> DvisorResult<&Box<dyn ITableFragmentProvider>> {
        if !self.providers.contains_key(from) {
            let ds = self.requester.create_data_source_with_metadata(from)?;
            self.providers.insert(from.to_string(), Rc::new(Box::new(ds)));
        }
        Ok(self.providers.get(from).unwrap())
    }

    pub fn exec(&mut self, query: &str) -> DvisorResult<QueryResult> {
        let tid = self.thread_id;
        let tc = self.thread_count;
        let q = Query::new(query)?;
        let plan = q.generate_plan()?;
        let from = q.from.clone();
        plan.execute(self,tid, tc, from)
    }

    pub fn create_metadata(&self, name: &str) -> DvisorResult<Metadata> {
        self.requester.create_metadata(name)
    }

}
