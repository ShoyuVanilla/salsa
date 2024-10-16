pub(crate) fn supertype(args: proc_macro::TokenStream) -> proc_macro::TokenStream {
    todo!()
}

pub struct SupertypeEnum;

impl crate::options::AllowedOptions for SupertypeEnum {
    const RETURN_REF: bool = false;

    const SPECIFY: bool = false;

    const NO_EQ: bool = false;

    const NO_DEBUG: bool = true;

    const NO_CLONE: bool = false;

    const SINGLETON: bool = true;

    const DATA: bool = true;

    const DB: bool = false;

    const RECOVERY_FN: bool = false;

    const LRU: bool = false;

    const CONSTRUCTOR_NAME: bool = true;
}
