use std::collections::BTreeMap;

use reqwest::blocking::{Client, Response};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{
    AuthorityDecision, AuthorityMode, MEMORY_DATA_VERSION, MemoryData, MemoryKind, MemoryScope,
    RecallOptions, RecallResult,
    error::{NahualiError, Result},
    recall,
};

include!("semantic/types.rs");
include!("semantic/index.rs");
include!("semantic/qdrant.rs");
include!("semantic/hybrid.rs");
include!("semantic/tests.rs");
