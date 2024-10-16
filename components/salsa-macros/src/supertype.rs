use proc_macro2::TokenStream;

use crate::hygiene::Hygiene;

pub(crate) fn supertype(
    _args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let hygiene = Hygiene::from1(&input);
    let enum_item = parse_macro_input!(input as syn::ItemEnum);
    todo!()
}

struct Macro {
    hygiene: Hygiene,
    enum_item: syn::ItemEnum,
}

impl Macro {
    fn try_macro(&self) -> syn::Result<TokenStream> {
        let attrs = &self.enum_item.attrs;
        let vis = &self.enum_item.vis;
        let enum_ident = &self.enum_item.ident;
        let variant_enum_name = self.hygiene.ident(&format!("{}Variants", enum_ident));
        let variants = &self.enum_item.variants.iter();

        let zalsa = self.hygiene.ident("zalsa");
        let zalsa_struct = self.hygiene.ident("zalsa_struct");
        let Configuration = self.hygiene.ident("Configuration");
        let Builder = self.hygiene.ident("Builder");
        let CACHE = self.hygiene.ident("CACHE");
        let Db = self.hygiene.ident("Db");

        todo!()
    }
}
