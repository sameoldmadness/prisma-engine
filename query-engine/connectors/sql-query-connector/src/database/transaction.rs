use crate::database::operations::*;
use crate::{query_builder::read::ManyRelatedRecordsQueryBuilder, SqlError};
use connector_interface::{
    self as connector,
    filter::{Filter, RecordFinder},
    QueryArguments, ReadOperations, ScalarListValues, Transaction, WriteArgs, WriteOperations, IO
};
use prisma_models::prelude::*;
use std::marker::PhantomData;

pub struct SqlConnectorTransaction<'a, T> {
    inner: quaint::connector::Transaction<'a>,
    _p: PhantomData<T>,
}

impl<'a, T> SqlConnectorTransaction<'a, T> {
    pub fn new(tx: quaint::connector::Transaction<'a>) -> Self {
        Self {
            inner: tx,
            _p: PhantomData,
        }
    }
}

impl<'a, T> Transaction<'a> for SqlConnectorTransaction<'a, T>
where
    T: ManyRelatedRecordsQueryBuilder + Send + Sync + 'static,
{
    fn commit<'b>(&'b self) -> IO<'b, ()> {
        IO::new(async move { Ok(self.inner.commit().await.map_err(SqlError::from)?) })
    }

    fn rollback<'b>(&'b self) -> IO<'b, ()> {
        IO::new(async move { Ok(self.inner.rollback().await.map_err(SqlError::from)?) })
    }
}

impl<'a, T> ReadOperations for SqlConnectorTransaction<'a, T>
where
    T: ManyRelatedRecordsQueryBuilder + Send + Sync + 'static,
{
    fn get_single_record<'b>(
        &'b self,
        record_finder: &'b RecordFinder,
        selected_fields: &'b SelectedFields,
    ) -> connector::IO<'b, Option<SingleRecord>> {
        IO::new(async move { read::get_single_record(&self.inner, record_finder, selected_fields).await })
    }

    fn get_many_records<'b>(
        &'b self,
        model: &'b ModelRef,
        query_arguments: QueryArguments,
        selected_fields: &'b SelectedFields,
    ) -> connector::IO<'b, ManyRecords> {
        IO::new(async move { read::get_many_records(&self.inner, model, query_arguments, selected_fields).await })
    }

    fn get_related_records<'b>(
        &'b self,
        from_field: &'b RelationFieldRef,
        from_record_ids: &'b [GraphqlId],
        query_arguments: QueryArguments,
        selected_fields: &'b SelectedFields,
    ) -> connector::IO<'b, ManyRecords> {
        IO::new(async move {
            read::get_related_records::<T>(
                &self.inner,
                from_field,
                from_record_ids,
                query_arguments,
                selected_fields,
            )
            .await
        })
    }

    fn get_scalar_list_values<'b>(
        &'b self,
        list_field: &'b ScalarFieldRef,
        record_ids: Vec<GraphqlId>,
    ) -> connector::IO<'b, Vec<ScalarListValues>> {
        IO::new(async move { read::get_scalar_list_values(&self.inner, list_field, record_ids).await })
    }

    fn count_by_model<'b>(&'b self, model: &'b ModelRef, query_arguments: QueryArguments) -> connector::IO<'b, usize> {
        IO::new(async move { read::count_by_model(&self.inner, model, query_arguments).await })
    }
}

impl<'a, T> WriteOperations for SqlConnectorTransaction<'a, T>
where
    T: ManyRelatedRecordsQueryBuilder + Send + Sync + 'static,
{
    fn create_record<'b>(&'b self, model: &'b ModelRef, args: WriteArgs) -> connector::IO<GraphqlId> {
        IO::new(async move { write::create_record(&self.inner, model, args).await })
    }

    fn update_records<'b>(&'b self, model: &'b ModelRef, where_: Filter, args: WriteArgs) -> connector::IO<Vec<GraphqlId>> {
        IO::new(async move { write::update_records(&self.inner, model, where_, args).await })
    }

    fn delete_records<'b>(&'b self, model: &'b ModelRef, where_: Filter) -> connector::IO<usize> {
        IO::new(async move { write::delete_records(&self.inner, model, where_).await })
    }

    fn connect<'b>(
        &'b self,
        field: &'b RelationFieldRef,
        parent_id: &'b GraphqlId,
        child_id: &'b GraphqlId,
    ) -> connector::IO<()> {
        IO::new(async move { write::connect(&self.inner, field, parent_id, child_id).await })
    }

    fn disconnect<'b>(
        &'b self,
        field: &'b RelationFieldRef,
        parent_id: &'b GraphqlId,
        child_id: &'b GraphqlId,
    ) -> connector::IO<()> {
        IO::new(async move { write::disconnect(&self.inner, field, parent_id, child_id).await })
    }

    fn set<'b>(
        &'b self,
        relation_field: &'b RelationFieldRef,
        parent_id: GraphqlId,
        wheres: Vec<GraphqlId>,
    ) -> connector::IO<()> {
        IO::new(async move { write::set(&self.inner, relation_field, parent_id, wheres).await })
    }
}
