use serde::{Deserialize, Serialize};
use std::fmt;
use try_match::try_match;

pub mod visit_mut;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Crate {
    pub comps: Vec<CompDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Visibility {
    Private,
    /// This variant is used only for the current crate. Replaced with
    /// `Private` when exporting the metadata to a file in accordance with
    /// the one-crate-one-file rule.
    Restricted(Path),
    Public,
}

/// The absolute path to an item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Path {
    pub root: PathRoot,
    pub idents: Vec<Ident>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PathRoot {
    Crate,
}

pub type Ident = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompDef {
    pub flags: CompFlags,
    pub vis: Visibility,
    /// The path of the component's type. Note that a component can have
    /// multiple aliases.
    pub paths: Vec<Path>,
    pub items: Vec<CompItemDef>,
}

bitflags::bitflags! {
    #[derive(Serialize, Deserialize)]
    pub struct CompFlags: u8 {
        /// Do not generate implementation code.
        const PROTOTYPE_ONLY = 1;

        /// The component represents a widget.
        const WIDGET = 1 << 1;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompItemDef {
    Field(FieldDef),
    Event(EventDef),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDef {
    pub field_ty: FieldType,
    pub flags: FieldFlags,
    pub ident: Ident,
    pub accessors: FieldAccessors,
    /// `Some(_)` if the field type refers to a component. `None` otherwise.
    pub ty: Option<CompRef>,
}

bitflags::bitflags! {
    #[derive(Serialize, Deserialize)]
    pub struct FieldFlags: u8 {
        const INJECT = 1;

        /// Only valid in `metadata`. `field_ty` must be `Const`.
        const OPTIONAL = 1 << 1;
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum FieldType {
    Prop,
    Const,
    Wire,
}

/// References a `CompDef` in `Crate`. (TODO: support referencing compoents
/// from other crates)
pub type CompRef = usize;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldAccessors {
    /// Valid only for `prop`
    pub set: Option<FieldSetter>,
    /// Valid for all field types
    pub get: Option<FieldGetter>,
    /// Valid only for `prop` and `wire`
    pub watch: Option<FieldWatcher>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldSetter {
    pub vis: Visibility,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldGetter {
    pub vis: Visibility,
    pub mode: FieldGetMode,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum FieldGetMode {
    /// The getter returns `impl Deref<Target = T>`.
    Borrow,
    /// The getter returns `T`.
    Clone,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldWatcher {
    pub vis: Visibility,
    /// Refers to an event in the same component where the field is defined.
    pub event: Ident,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventDef {
    pub vis: Visibility,
    pub ident: Ident,
    pub inputs: Vec<Ident>,
}

// Printing
// ---------------------------------------------------------------------------

impl fmt::Display for Path {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_ref().fmt(f)
    }
}

impl fmt::Display for PathRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "crate")?;
        for ident in self.idents.iter() {
            write!(f, "::{}", ident)?;
        }
        Ok(())
    }
}

impl fmt::Display for Visibility {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_ref().fmt(f)
    }
}

impl fmt::Display for VisibilityRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VisibilityRef::Private => Ok(()),
            VisibilityRef::Restricted(p) => write!(f, "pub (in {})", p),
            VisibilityRef::Public => write!(f, "pub"),
        }
    }
}

// Metadata Manipulation
// ---------------------------------------------------------------------------

/// The borrowed version of `Visiblity`.
#[derive(Debug, Clone)]
pub enum VisibilityRef<'a> {
    Private,
    Restricted(PathRef<'a>),
    Public,
}

impl Visibility {
    pub fn as_ref(&self) -> VisibilityRef<'_> {
        match self {
            Visibility::Private => VisibilityRef::Private,
            Visibility::Restricted(p) => VisibilityRef::Restricted(p.as_ref()),
            Visibility::Public => VisibilityRef::Public,
        }
    }
}

impl VisibilityRef<'_> {
    pub fn strictest(self, other: Self) -> Self {
        match (self, other) {
            (Self::Private, _) => Self::Private,
            (_, Self::Private) => Self::Private,
            (Self::Restricted(p), Self::Public) => Self::Restricted(p),
            (Self::Public, Self::Restricted(p)) => Self::Restricted(p),
            (Self::Restricted(p1), Self::Restricted(p2)) => {
                if let Some(p) = p1.lowest_common_ancestor(&p2) {
                    Self::Restricted(p)
                } else {
                    Self::Private
                }
            }
            (Self::Public, Self::Public) => Self::Public,
        }
    }
}

/// The borrowed version of `Path`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathRef<'a> {
    root: PathRoot,
    idents: &'a [Ident],
}

impl Path {
    pub fn as_ref(&self) -> PathRef<'_> {
        PathRef {
            root: self.root,
            idents: &self.idents[..],
        }
    }
}

impl PathRef<'_> {
    pub fn to_owned(&self) -> Path {
        Path {
            root: self.root,
            idents: self.idents.to_owned(),
        }
    }

    pub fn parent(&self) -> Option<Self> {
        if self.idents.is_empty() {
            None
        } else {
            Some(Self {
                root: self.root,
                idents: &self.idents[..self.idents.len() - 1],
            })
        }
    }

    pub fn starts_with(&self, other: &PathRef<'_>) -> bool {
        self.root == other.root && self.idents.starts_with(other.idents)
    }

    pub fn lowest_common_ancestor(&self, other: &Self) -> Option<Self> {
        if self.root == other.root {
            let len = self
                .idents
                .iter()
                .zip(other.idents.iter())
                .take_while(|(a, b)| a == b)
                .count();
            Some(Self {
                root: self.root,
                idents: &self.idents[..len],
            })
        } else {
            None
        }
    }
}

impl CompItemDef {
    pub fn event(&self) -> Option<&EventDef> {
        try_match!(Self::Event(event) = self).ok()
    }

    pub fn ident(&self) -> &Ident {
        match self {
            CompItemDef::Field(field) => &field.ident,
            CompItemDef::Event(event) => &event.ident,
        }
    }
}

impl CompDef {
    /// Calculate the maximum possibile visibility of the component's builder
    /// type can have. Having a visibility beyond this is pointless on account
    /// of `const` fields that can't be initialized.
    pub fn builder_vis(&self) -> VisibilityRef<'_> {
        self.items
            .iter()
            .filter_map(|item| match item {
                // Non-optional `const` fields have
                CompItemDef::Field(FieldDef {
                    field_ty: FieldType::Const,
                    flags,
                    accessors:
                        FieldAccessors {
                            set: Some(FieldSetter { vis }),
                            ..
                        },
                    ..
                }) if !flags.contains(FieldFlags::OPTIONAL) => Some(vis.as_ref()),
                _ => None,
            })
            .fold(VisibilityRef::Public, VisibilityRef::strictest)
    }

    pub fn find_item_by_ident(&self, ident: &str) -> Option<(usize, &CompItemDef)> {
        self.items
            .iter()
            .enumerate()
            .find(|(_, item)| item.ident() == ident)
    }
}
