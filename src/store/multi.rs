// Copyright 2018 Mozilla
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use
// this file except in compliance with the License. You may obtain a copy of the
// License at http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software distributed
// under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
// CONDITIONS OF ANY KIND, either express or implied. See the License for the
// specific language governing permissions and limitations under the License.

use std::marker::PhantomData;

use crate::backend::{
    BackendDatabase,
    BackendFlags,
    BackendIter,
    BackendRoCursor,
    BackendRwTransaction,
};
use crate::error::StoreError;
use crate::helpers::read_transform;
use crate::readwrite::{
    Readable,
    Writer,
};
use crate::value::Value;

type EmptyResult = Result<(), StoreError>;

#[derive(Copy, Clone)]
pub struct MultiStore<D> {
    db: D,
}

pub struct Iter<'env, I, C> {
    iter: I,
    cursor: C,
    phantom: PhantomData<&'env ()>,
}

impl<D> MultiStore<D>
where
    D: BackendDatabase,
{
    pub(crate) fn new(db: D) -> MultiStore<D> {
        MultiStore {
            db,
        }
    }

    /// Provides a cursor to all of the values for the duplicate entries that match this key
    pub fn get<'env, R, I, C, K>(self, reader: &'env R, k: K) -> Result<Iter<'env, I, C>, StoreError>
    where
        R: Readable<'env, Database = D, RoCursor = C>,
        I: BackendIter<'env>,
        C: BackendRoCursor<'env, Iter = I>,
        K: AsRef<[u8]>,
    {
        let mut cursor = reader.open_ro_cursor(self.db)?;
        let iter = cursor.iter_dup_of(k);

        Ok(Iter {
            iter,
            cursor,
            phantom: PhantomData,
        })
    }

    /// Provides the first value that matches this key
    pub fn get_first<'env, R, K>(self, reader: &'env R, k: K) -> Result<Option<Value<'env>>, StoreError>
    where
        R: Readable<'env, Database = D>,
        K: AsRef<[u8]>,
    {
        reader.get(self.db, &k)
    }

    /// Insert a value at the specified key.
    /// This put will allow duplicate entries.  If you wish to have duplicate entries
    /// rejected, use the `put_with_flags` function and specify NO_DUP_DATA
    pub fn put<T, K>(self, writer: &mut Writer<T>, k: K, v: &Value) -> EmptyResult
    where
        T: BackendRwTransaction<Database = D>,
        K: AsRef<[u8]>,
    {
        writer.put(self.db, &k, v, T::Flags::empty())
    }

    pub fn put_with_flags<T, K>(self, writer: &mut Writer<T>, k: K, v: &Value, flags: T::Flags) -> EmptyResult
    where
        T: BackendRwTransaction<Database = D>,
        K: AsRef<[u8]>,
    {
        writer.put(self.db, &k, v, flags)
    }

    pub fn delete_all<T, K>(self, writer: &mut Writer<T>, k: K) -> EmptyResult
    where
        T: BackendRwTransaction<Database = D>,
        K: AsRef<[u8]>,
    {
        writer.delete(self.db, &k, None)
    }

    pub fn delete<T, K>(self, writer: &mut Writer<T>, k: K, v: &Value) -> EmptyResult
    where
        T: BackendRwTransaction<Database = D>,
        K: AsRef<[u8]>,
    {
        writer.delete(self.db, &k, Some(&v.to_bytes()?))
    }

    pub fn clear<T>(self, writer: &mut Writer<T>) -> EmptyResult
    where
        T: BackendRwTransaction<Database = D>,
    {
        writer.clear(self.db)
    }
}

impl<'env, I, C> Iterator for Iter<'env, I, C>
where
    I: BackendIter<'env>,
    C: BackendRoCursor<'env, Iter = I>,
{
    type Item = Result<(&'env [u8], Option<Value<'env>>), StoreError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.iter.next() {
            None => None,
            Some(Ok((key, bytes))) => match read_transform(Ok(bytes)) {
                Ok(val) => Some(Ok((key, val))),
                Err(err) => Some(Err(err)),
            },
            Some(Err(err)) => Some(Err(err.into())),
        }
    }
}
