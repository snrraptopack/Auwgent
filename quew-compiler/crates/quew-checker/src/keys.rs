use quew_interner::{InternedStr, Interner};
use quew_types::Ty;

/// Keys for validating `reply(...) with { ... }` fields and temporary built-in
/// method experiments before real `extend` method tables exist.
pub(crate) struct WellKnownKeys {
    pub(crate) model: InternedStr,
    pub(crate) fallback: InternedStr,
    pub(crate) prompt: InternedStr,
    pub(crate) tools: InternedStr,
    pub(crate) retry: InternedStr,
    pub(crate) max_turn: InternedStr,
    pub(crate) ctx: InternedStr,
    pub(crate) is_empty: InternedStr,
    pub(crate) tool: InternedStr,
    pub(crate) value: InternedStr,
    pub(crate) with: InternedStr,
    pub(crate) body: InternedStr,
    pub(crate) builtin: InternedStr,
}

impl WellKnownKeys {
    pub(crate) fn new(i: &Interner) -> Self {
        Self {
            model: i.intern("model"),
            fallback: i.intern("fallback"),
            prompt: i.intern("prompt"),
            tools: i.intern("tools"),
            retry: i.intern("retry"),
            max_turn: i.intern("maxTurn"),
            ctx: i.intern("ctx"),
            is_empty: i.intern("isEmpty"),
            tool: i.intern("tool"),
            value: i.intern("value"),
            with: i.intern("with"),
            body: i.intern("body"),
            builtin: i.intern("builtin"),
        }
    }
}

/// Primitive type name -> `Ty` mapping.
pub(crate) struct PrimKeys {
    string: InternedStr,
    number: InternedStr,
    bool_k: InternedStr,
    float: InternedStr,
    null: InternedStr,
    void: InternedStr,
    text: InternedStr,
}

impl PrimKeys {
    pub(crate) fn new(i: &Interner) -> Self {
        Self {
            string: i.intern("string"),
            number: i.intern("number"),
            bool_k: i.intern("bool"),
            float: i.intern("float"),
            null: i.intern("null"),
            void: i.intern("void"),
            text: i.intern("Text"),
        }
    }

    pub(crate) fn resolve(&self, name: InternedStr) -> Option<Ty> {
        if name == self.string || name == self.text {
            Some(Ty::string())
        } else if name == self.number {
            Some(Ty::number())
        } else if name == self.bool_k {
            Some(Ty::bool_ty())
        } else if name == self.float {
            Some(Ty::float())
        } else if name == self.null {
            Some(Ty::null())
        } else if name == self.void {
            Some(Ty::void())
        } else {
            None
        }
    }
}
