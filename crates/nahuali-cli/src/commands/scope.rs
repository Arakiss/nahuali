use anyhow::Context;
use nahuali_core::MemoryScope;

pub(crate) fn parse_scope(value: Option<String>) -> anyhow::Result<Option<MemoryScope>> {
    value
        .map(|value| {
            MemoryScope::parse(&value).with_context(|| {
                format!(
                    "invalid --scope {value:?}; expected kind:name, for example project:nahuali"
                )
            })
        })
        .transpose()
}
