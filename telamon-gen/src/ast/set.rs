use super::*;

#[derive(Debug, Clone)]
pub struct SetDef {
    pub name: Spanned<String>,
    pub doc: Option<String>,
    pub arg: Option<VarDef>,
    pub superset: Option<SetRef>,
    pub disjoint: Vec<String>,
    pub keys: Vec<(Spanned<ir::SetDefKey>, Option<VarDef>, String)>,
    pub quotient: Option<Quotient>,
}

impl SetDef {

    /// This checks that thereisn't any keys doublon.
    fn check_declare(&self) -> Result<(), TypeError> {
        let mut hash: HashMap<_, Spanned<()>> = HashMap::default();
        for (key, ..) in self.keys.iter() {
            if let Some(pre) = hash.insert(key.data.to_owned(), key.with_data(())) {
                Err(TypeError::Redefinition(
                    pre.with_data(Hint::Set),
                    key.with_data(key.data.to_string()),
                ))?;
            }
        }
        Ok(())
    }

    /// This checks the presence of keys ItemType, IdType, ItemGetter, IdGetter and Iter.
    /// When there is a superset, this checks too the presence of FromSuperset keyword.
    fn check_missing_entry(&self) -> Result<(), TypeError> {
        let keys = self.keys.iter()
                            .map(|(k, _, _)| k.data)
                            .collect::<Vec<ir::SetDefKey>>();

        if !keys.contains(&&ir::SetDefKey::ItemType) {
            Err(TypeError::MissingEntry(self.name.data.to_owned(),
                self.name.with_data(ir::SetDefKey::ItemType.to_string())))?;
        }
        if !keys.contains(&&ir::SetDefKey::IdType) {
            Err(TypeError::MissingEntry(self.name.data.to_owned(),
                self.name.with_data(ir::SetDefKey::IdType.to_string())))?;
        }
        if !keys.contains(&&ir::SetDefKey::ItemGetter) {
            Err(TypeError::MissingEntry(self.name.data.to_owned(),
                self.name.with_data(ir::SetDefKey::ItemGetter.to_string())))?;
        }
        if !keys.contains(&&ir::SetDefKey::IdGetter) {
            Err(TypeError::MissingEntry(self.name.data.to_owned(),
                self.name.with_data(ir::SetDefKey::IdGetter.to_string())))?;
        }
        if !keys.contains(&&ir::SetDefKey::Iter) {
            Err(TypeError::MissingEntry(self.name.data.to_owned(),
                self.name.with_data(ir::SetDefKey::Iter.to_string())))?;
        }
        if self.superset.is_some() && !keys.contains(&&ir::SetDefKey::FromSuperset) {
            Err(TypeError::MissingEntry(self.name.data.to_owned(),
                self.name.with_data(ir::SetDefKey::FromSuperset.to_string())))?;
        }
        Ok(())
    }

    /// Type checks the declare's condition.
    pub fn declare(&self) -> Result<(), TypeError> {
        self.check_declare()?;
        Ok(())
    }

    /// Type checks the define's condition.
    pub fn define(&self) -> Result<(), TypeError> {
        self.check_missing_entry()?;
        Ok(())
    }
}

impl PartialEq for SetDef {
    fn eq(&self, rhs: &Self) -> bool {
        self.name == rhs.name
    }
}
