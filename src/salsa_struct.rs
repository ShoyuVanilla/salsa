use crate::{Database, Id, IngredientIndex};

pub trait SalsaStruct<'db> {
    fn new(db: &'db dyn Database, id: Id) -> Self;
    fn ingredient_index(db: &'db dyn Database) -> IngredientIndex;
}
