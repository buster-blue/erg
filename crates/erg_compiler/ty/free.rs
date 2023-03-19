use std::cell::{Ref, RefMut};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::mem;

use erg_common::shared::Shared;
use erg_common::traits::{LimitedDisplay, StructuralEq};
use erg_common::Str;
use erg_common::{addr_eq, log};

use super::typaram::TyParam;
use super::Type;

pub type Level = usize;
pub type Id = usize;

/// HACK: see doc/compiler/inference.md for details
pub const GENERIC_LEVEL: usize = usize::MAX;

thread_local! {
    static UNBOUND_ID: Shared<usize> = Shared::new(0);
}

pub trait HasLevel {
    fn level(&self) -> Option<Level>;
    fn set_level(&self, lev: Level);
    fn lower(&self, level: Level) {
        if self.level() < Some(level) {
            self.set_level(level);
        }
    }
    fn lift(&self) {
        if let Some(lev) = self.level() {
            self.set_level(lev.saturating_add(1));
        }
    }
    fn generalize(&self) {
        self.set_level(GENERIC_LEVEL);
    }
    fn is_generalized(&self) -> bool {
        self.level() == Some(GENERIC_LEVEL)
    }
}

/// Represents constraints on type variables and type parameters.
///
/// Note that constraints can have circular references. However, type variable (`FreeTyVar`) is defined with various operations avoiding infinite recursion.
///
/// __NOTE__: you should use `Free::get_type/get_subsup` instead of deconstructing the constraint by `match`.
/// Constraints may contain cycles, in which case using `match` to get the contents will cause memory destructions.
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Constraint {
    // : Type --> (:> Never, <: Obj)
    // :> Sub --> (:> Sub, <: Obj)
    // <: Sup --> (:> Never, <: Sup)
    /// :> Sub, <: Sup
    Sandwiched {
        sub: Type,
        sup: Type,
    },
    // : Int, ...
    TypeOf(Type),
    Uninited,
}

impl fmt::Display for Constraint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.limited_fmt(f, 10)
    }
}

impl fmt::Debug for Constraint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.limited_fmt(f, 10)
    }
}

impl LimitedDisplay for Constraint {
    fn limited_fmt(&self, f: &mut fmt::Formatter<'_>, limit: usize) -> fmt::Result {
        if limit == 0 {
            return write!(f, "...");
        }
        match self {
            Self::Sandwiched { sub, sup } => match (sub == &Type::Never, sup == &Type::Obj) {
                (true, true) => {
                    write!(f, ": Type")?;
                    if cfg!(feature = "debug") {
                        write!(f, "(:> Never, <: Obj)")?;
                    }
                    Ok(())
                }
                (true, false) => {
                    write!(f, "<: ")?;
                    sup.limited_fmt(f, limit - 1)?;
                    Ok(())
                }
                (false, true) => {
                    write!(f, ":> ")?;
                    sub.limited_fmt(f, limit - 1)?;
                    Ok(())
                }
                (false, false) => {
                    write!(f, ":> ")?;
                    sub.limited_fmt(f, limit - 1)?;
                    write!(f, ", <: ")?;
                    sup.limited_fmt(f, limit - 1)?;
                    Ok(())
                }
            },
            Self::TypeOf(t) => {
                write!(f, ": ")?;
                t.limited_fmt(f, limit - 1)
            }
            Self::Uninited => write!(f, "<uninited>"),
        }
    }
}

impl Constraint {
    /// :> Sub, <: Sup
    pub const fn new_sandwiched(sub: Type, sup: Type) -> Self {
        Self::Sandwiched { sub, sup }
    }

    pub fn named_fmt(&self, f: &mut fmt::Formatter<'_>, name: &str, limit: usize) -> fmt::Result {
        if limit == 0 {
            return write!(f, "...");
        }
        match self {
            Self::Sandwiched { sub, sup } => match (sub == &Type::Never, sup == &Type::Obj) {
                (true, true) => {
                    write!(f, "{name}: Type")?;
                    Ok(())
                }
                (true, false) => {
                    write!(f, "{name} <: ")?;
                    sup.limited_fmt(f, limit - 1)?;
                    Ok(())
                }
                (false, true) => {
                    write!(f, "{name} :> ")?;
                    sub.limited_fmt(f, limit - 1)?;
                    Ok(())
                }
                (false, false) => {
                    write!(f, "{name} :> ")?;
                    sub.limited_fmt(f, limit - 1)?;
                    write!(f, ", {name} <: ")?;
                    sup.limited_fmt(f, limit - 1)?;
                    Ok(())
                }
            },
            Self::TypeOf(t) => {
                write!(f, "{name}: ")?;
                t.limited_fmt(f, limit - 1)
            }
            Self::Uninited => write!(f, "Never"),
        }
    }

    pub fn new_type_of(t: Type) -> Self {
        if t == Type::Type {
            Self::new_sandwiched(Type::Never, Type::Obj)
        } else {
            Self::TypeOf(t)
        }
    }

    pub const fn new_subtype_of(sup: Type) -> Self {
        Self::new_sandwiched(Type::Never, sup)
    }

    pub const fn new_supertype_of(sub: Type) -> Self {
        Self::new_sandwiched(sub, Type::Obj)
    }

    pub const fn is_uninited(&self) -> bool {
        matches!(self, Self::Uninited)
    }

    pub fn lift(&self) {
        match self {
            Self::Sandwiched { sub, sup, .. } => {
                sub.lift();
                sup.lift();
            }
            Self::TypeOf(t) => t.lift(),
            Self::Uninited => {}
        }
    }

    pub fn get_type(&self) -> Option<&Type> {
        match self {
            Self::TypeOf(ty) => Some(ty),
            Self::Sandwiched {
                sub: Type::Never,
                sup: Type::Obj,
                ..
            } => Some(&Type::Type),
            _ => None,
        }
    }

    /// :> Sub
    pub fn get_sub(&self) -> Option<&Type> {
        match self {
            Self::Sandwiched { sub, .. } => Some(sub),
            _ => None,
        }
    }

    /// <: Sup
    pub fn get_super(&self) -> Option<&Type> {
        match self {
            Self::Sandwiched { sup, .. } => Some(sup),
            _ => None,
        }
    }

    pub fn get_sub_sup(&self) -> Option<(&Type, &Type)> {
        match self {
            Self::Sandwiched { sub, sup, .. } => Some((sub, sup)),
            _ => None,
        }
    }

    pub fn get_super_mut(&mut self) -> Option<&mut Type> {
        match self {
            Self::Sandwiched { sup, .. } => Some(sup),
            _ => None,
        }
    }
}

pub trait CanbeFree {
    fn unbound_name(&self) -> Option<Str>;
    fn constraint(&self) -> Option<Constraint>;
    fn update_constraint(&self, constraint: Constraint, in_instantiation: bool);
}

impl<T: CanbeFree> Free<T> {
    pub fn unbound_name(&self) -> Option<Str> {
        match &*self.borrow() {
            FreeKind::NamedUnbound { name, .. } => Some(name.clone()),
            FreeKind::Unbound { id, .. } => Some(Str::from(format!("%{id}"))),
            FreeKind::Linked(t) | FreeKind::UndoableLinked { t, .. } => t.unbound_name(),
        }
    }

    pub fn constraint(&self) -> Option<Constraint> {
        match &*self.borrow() {
            FreeKind::Unbound { constraint, .. } | FreeKind::NamedUnbound { constraint, .. } => {
                Some(constraint.clone())
            }
            FreeKind::Linked(t) | FreeKind::UndoableLinked { t, .. } => t.constraint(),
        }
    }
}

#[derive(Debug, Clone, Eq)]
pub enum FreeKind<T> {
    Linked(T),
    UndoableLinked {
        t: T,
        previous: Box<FreeKind<T>>,
    },
    Unbound {
        id: Id,
        lev: Level,
        constraint: Constraint,
    },
    NamedUnbound {
        name: Str,
        lev: Level,
        constraint: Constraint,
    },
}

impl<T: Hash> Hash for FreeKind<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Self::Linked(t) | Self::UndoableLinked { t, .. } => t.hash(state),
            Self::Unbound {
                id,
                lev,
                constraint,
            } => {
                id.hash(state);
                lev.hash(state);
                constraint.hash(state);
            }
            Self::NamedUnbound {
                name,
                lev,
                constraint,
            } => {
                name.hash(state);
                lev.hash(state);
                constraint.hash(state);
            }
        }
    }
}

impl<T: PartialEq> PartialEq for FreeKind<T> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Linked(t1) | Self::UndoableLinked { t: t1, .. },
                Self::Linked(t2) | Self::UndoableLinked { t: t2, .. },
            ) => t1 == t2,
            (
                Self::Unbound {
                    id: id1,
                    lev: lev1,
                    constraint: c1,
                },
                Self::Unbound {
                    id: id2,
                    lev: lev2,
                    constraint: c2,
                },
            ) => id1 == id2 && lev1 == lev2 && c1 == c2,
            (
                Self::NamedUnbound {
                    name: n1,
                    lev: l1,
                    constraint: c1,
                },
                Self::NamedUnbound {
                    name: n2,
                    lev: l2,
                    constraint: c2,
                },
            ) => n1 == n2 && l1 == l2 && c1 == c2,
            _ => false,
        }
    }
}

impl<T: LimitedDisplay> fmt::Display for FreeKind<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.limited_fmt(f, 10)
    }
}

impl<T: LimitedDisplay> LimitedDisplay for FreeKind<T> {
    fn limited_fmt(&self, f: &mut fmt::Formatter<'_>, limit: usize) -> fmt::Result {
        if limit == 0 {
            return write!(f, "...");
        }
        match self {
            Self::Linked(t) | Self::UndoableLinked { t, .. } => {
                if cfg!(feature = "debug") {
                    write!(f, "(")?;
                    t.limited_fmt(f, limit)?;
                    write!(f, ")")
                } else {
                    t.limited_fmt(f, limit)
                }
            }
            Self::NamedUnbound {
                name,
                lev,
                constraint,
            } => {
                if *lev == GENERIC_LEVEL {
                    write!(f, "{name}")?;
                    if cfg!(feature = "debug") {
                        write!(f, "(")?;
                        constraint.limited_fmt(f, limit - 1)?;
                        write!(f, ")")?;
                    }
                } else {
                    write!(f, "?{name}")?;
                    if cfg!(feature = "debug") {
                        write!(f, "(")?;
                        constraint.limited_fmt(f, limit - 1)?;
                        write!(f, ")")?;
                        write!(f, "[{lev}]")?;
                    }
                }
                Ok(())
            }
            Self::Unbound {
                id,
                lev,
                constraint,
            } => {
                if *lev == GENERIC_LEVEL {
                    write!(f, "%{id}")?;
                    if cfg!(feature = "debug") {
                        write!(f, "(")?;
                        constraint.limited_fmt(f, limit - 1)?;
                        write!(f, ")")?;
                    }
                } else {
                    write!(f, "?{id}")?;
                    if cfg!(feature = "debug") {
                        write!(f, "(")?;
                        constraint.limited_fmt(f, limit - 1)?;
                        write!(f, ")")?;
                        write!(f, "[{lev}]")?;
                    }
                }
                Ok(())
            }
        }
    }
}

impl<T> FreeKind<T> {
    pub const fn unbound(id: Id, lev: Level, constraint: Constraint) -> Self {
        Self::Unbound {
            id,
            lev,
            constraint,
        }
    }

    pub fn new_unbound(lev: Level, constraint: Constraint) -> Self {
        UNBOUND_ID.with(|id| {
            *id.borrow_mut() += 1;
            Self::Unbound {
                id: *id.borrow(),
                lev,
                constraint,
            }
        })
    }

    pub const fn named_unbound(name: Str, lev: Level, constraint: Constraint) -> Self {
        Self::NamedUnbound {
            name,
            lev,
            constraint,
        }
    }

    pub const fn linked(&self) -> Option<&T> {
        match self {
            Self::Linked(typ) => Some(typ),
            _ => None,
        }
    }

    pub fn linked_mut(&mut self) -> Option<&mut T> {
        match self {
            Self::Linked(typ) => Some(typ),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Free<T>(Shared<FreeKind<T>>);

impl Hash for Free<Type> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        if let Some(name) = self.unbound_name() {
            name.hash(state);
        }
        if let Some(lev) = self.level() {
            lev.hash(state);
        }
        if let Some((sub, sup)) = self.get_subsup() {
            self.forced_undoable_link(&sub);
            sub.hash(state);
            sup.hash(state);
            self.undo();
        } else if let Some(t) = self.get_type() {
            t.hash(state);
        } else if self.is_linked() {
            self.crack().hash(state);
        }
    }
}

impl Hash for Free<TyParam> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        if let Some(name) = self.unbound_name() {
            name.hash(state);
        }
        if let Some(lev) = self.level() {
            lev.hash(state);
        }
        if let Some(t) = self.get_type() {
            t.hash(state);
        } else if self.is_linked() {
            self.crack().hash(state);
        }
    }
}

impl PartialEq for Free<Type> {
    fn eq(&self, other: &Self) -> bool {
        if let Some((self_name, other_name)) = self.unbound_name().zip(other.unbound_name()) {
            if self_name != other_name {
                return false;
            }
        }
        if let Some((self_lev, other_lev)) = self.level().zip(other.level()) {
            if self_lev != other_lev {
                return false;
            }
        }
        if let Some((sub, sup)) = self.get_subsup() {
            if let Some((other_sub, other_sup)) = other.get_subsup() {
                self.forced_undoable_link(&sub);
                let res = sub == other_sub && sup == other_sup;
                self.undo();
                return res;
            }
        } else if let Some((self_t, other_t)) = self.get_type().zip(other.get_type()) {
            return self_t == other_t;
        } else if self.is_linked() && other.is_linked() {
            return self.crack().eq(&other.crack());
        }
        false
    }
}

impl PartialEq for Free<TyParam> {
    fn eq(&self, other: &Self) -> bool {
        if let Some((self_name, other_name)) = self.unbound_name().zip(other.unbound_name()) {
            if self_name != other_name {
                return false;
            }
        }
        if let Some((self_lev, other_lev)) = self.level().zip(other.level()) {
            if self_lev != other_lev {
                return false;
            }
        }
        if let Some((self_t, other_t)) = self.get_type().zip(other.get_type()) {
            return self_t == other_t;
        } else if self.is_linked() && other.is_linked() {
            return self.crack().eq(&other.crack());
        }
        false
    }
}

impl Eq for Free<Type> {}
impl Eq for Free<TyParam> {}

impl<T: LimitedDisplay> fmt::Display for Free<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.borrow())
    }
}

impl<T: LimitedDisplay> LimitedDisplay for Free<T> {
    fn limited_fmt(&self, f: &mut fmt::Formatter<'_>, limit: usize) -> fmt::Result {
        self.0.borrow().limited_fmt(f, limit)
    }
}

impl<T> Free<T> {
    pub fn borrow(&self) -> Ref<'_, FreeKind<T>> {
        self.0.borrow()
    }
    pub fn borrow_mut(&self) -> RefMut<'_, FreeKind<T>> {
        self.0.borrow_mut()
    }
    /// very unsafe, use `force_replace` instead whenever possible
    pub fn as_ptr(&self) -> *mut FreeKind<T> {
        self.0.as_ptr()
    }
    pub fn forced_as_ref(&self) -> &FreeKind<T> {
        unsafe { self.as_ptr().as_ref() }.unwrap()
    }
    pub fn force_replace(&self, new: FreeKind<T>) {
        // prevent linking to self
        if addr_eq!(*self.borrow(), new) {
            return;
        }
        unsafe {
            *self.0.as_ptr() = new;
        }
    }
    pub fn can_borrow(&self) -> bool {
        self.0.can_borrow()
    }
    pub fn can_borrow_mut(&self) -> bool {
        self.0.can_borrow_mut()
    }
}

impl<T: StructuralEq + CanbeFree + Clone + Default> StructuralEq for Free<T> {
    fn structural_eq(&self, other: &Self) -> bool {
        if let (Some((l, r)), Some((l2, r2))) = (self.get_subsup(), other.get_subsup()) {
            self.forced_undoable_link(&T::default());
            let res = l.structural_eq(&l2) && r.structural_eq(&r2);
            self.undo();
            res
        } else if let (Some(l), Some(r)) = (self.get_type(), other.get_type()) {
            l.structural_eq(&r)
        } else {
            self.constraint_is_uninited() && other.constraint_is_uninited()
        }
    }
}

impl<T: Clone> Free<T> {
    pub fn clone_inner(&self) -> FreeKind<T> {
        self.0.clone_inner()
    }
}

impl HasLevel for Free<Type> {
    fn set_level(&self, level: Level) {
        match unsafe { &mut *self.as_ptr() as &mut FreeKind<Type> } {
            FreeKind::Unbound { lev, .. } | FreeKind::NamedUnbound { lev, .. } => {
                if addr_eq!(*lev, level) {
                    return;
                }
                *lev = level;
                if let Some((sub, sup)) = self.get_subsup() {
                    self.forced_undoable_link(&sub);
                    sub.set_level(level);
                    sup.set_level(level);
                    self.undo();
                } else if let Some(t) = self.get_type() {
                    t.set_level(level);
                }
            }
            FreeKind::Linked(t) => {
                t.set_level(level);
            }
            _ => {}
        }
    }

    fn level(&self) -> Option<Level> {
        match &*self.borrow() {
            FreeKind::Unbound { lev, .. } | FreeKind::NamedUnbound { lev, .. } => Some(*lev),
            FreeKind::Linked(t) | FreeKind::UndoableLinked { t, .. } => t.level(),
        }
    }
}

impl HasLevel for Free<TyParam> {
    fn set_level(&self, level: Level) {
        match unsafe { &mut *self.as_ptr() as &mut FreeKind<TyParam> } {
            FreeKind::Unbound { lev, .. } | FreeKind::NamedUnbound { lev, .. } => {
                if addr_eq!(*lev, level) {
                    return;
                }
                *lev = level;
                if let Some(t) = self.get_type() {
                    t.set_level(level);
                }
            }
            FreeKind::Linked(t) => {
                t.set_level(level);
            }
            _ => {}
        }
    }

    fn level(&self) -> Option<Level> {
        match &*self.borrow() {
            FreeKind::Unbound { lev, .. } | FreeKind::NamedUnbound { lev, .. } => Some(*lev),
            FreeKind::Linked(t) | FreeKind::UndoableLinked { t, .. } => t.level(),
        }
    }
}

impl<T> Free<T> {
    pub fn new(f: FreeKind<T>) -> Self {
        Self(Shared::new(f))
    }

    pub fn new_unbound(level: Level, constraint: Constraint) -> Self {
        UNBOUND_ID.with(|id| {
            *id.borrow_mut() += 1;
            Self(Shared::new(FreeKind::unbound(
                *id.borrow(),
                level,
                constraint,
            )))
        })
    }

    pub fn new_named_unbound(name: Str, level: Level, constraint: Constraint) -> Self {
        Self(Shared::new(FreeKind::named_unbound(
            name, level, constraint,
        )))
    }

    pub fn new_linked(t: T) -> Self {
        Self(Shared::new(FreeKind::Linked(t)))
    }

    pub fn replace(&self, to: FreeKind<T>) {
        // prevent linking to self
        if self.is_linked() && addr_eq!(*self.borrow(), to) {
            return;
        }
        *self.borrow_mut() = to;
    }

    /// returns linked type (panic if self is unbounded)
    /// NOTE: check by `.is_linked` before call
    pub fn crack(&self) -> Ref<'_, T> {
        Ref::map(self.borrow(), |f| match f {
            FreeKind::Linked(t) | FreeKind::UndoableLinked { t, .. } => t,
            FreeKind::Unbound { .. } | FreeKind::NamedUnbound { .. } => {
                panic!("the value is unbounded")
            }
        })
    }

    pub fn crack_constraint(&self) -> Ref<'_, Constraint> {
        Ref::map(self.borrow(), |f| match f {
            FreeKind::Linked(_) | FreeKind::UndoableLinked { .. } => panic!("the value is linked"),
            FreeKind::Unbound { constraint, .. } | FreeKind::NamedUnbound { constraint, .. } => {
                constraint
            }
        })
    }

    pub fn is_linked(&self) -> bool {
        matches!(
            &*self.borrow(),
            FreeKind::Linked(_) | FreeKind::UndoableLinked { .. }
        )
    }

    pub fn is_undoable_linked(&self) -> bool {
        matches!(&*self.borrow(), FreeKind::UndoableLinked { .. })
    }

    pub fn unsafe_crack(&self) -> &T {
        match unsafe { self.as_ptr().as_ref().unwrap() } {
            FreeKind::Linked(t) | FreeKind::UndoableLinked { t, .. } => t,
            FreeKind::Unbound { .. } | FreeKind::NamedUnbound { .. } => {
                panic!("the value is unbounded")
            }
        }
    }
}

impl<T: Clone> Free<T> {
    pub fn link(&self, to: &T) {
        // prevent linking to self
        if self.is_linked() && addr_eq!(*self.crack(), *to) {
            return;
        }
        *self.borrow_mut() = FreeKind::Linked(to.clone());
    }

    /// NOTE: Do not use this except to rewrite circular references.
    /// No reference to any type variable may be left behind when rewriting.
    /// However, `get_bound_types` is safe because it does not return references.
    pub fn forced_link(&self, to: &T) {
        // prevent linking to self
        if self.is_linked() && addr_eq!(*self.crack(), *to) {
            return;
        }
        unsafe {
            *self.as_ptr() = FreeKind::Linked(to.clone());
        }
    }

    pub fn undoable_link(&self, to: &T) {
        if self.is_linked() && addr_eq!(*self.crack(), *to) {
            panic!("link to self");
        }
        let prev = self.clone_inner();
        let new = FreeKind::UndoableLinked {
            t: to.clone(),
            previous: Box::new(prev),
        };
        *self.borrow_mut() = new;
    }

    /// NOTE: Do not use this except to rewrite circular references.
    /// No reference to any type variable may be left behind when rewriting.
    /// However, `get_bound_types` is safe because it does not return references.
    pub fn forced_undoable_link(&self, to: &T) {
        if self.is_linked() && addr_eq!(*self.crack(), *to) {
            panic!("link to self");
        }
        let prev = self.clone_inner();
        let new = FreeKind::UndoableLinked {
            t: to.clone(),
            previous: Box::new(prev),
        };
        self.force_replace(new);
    }

    pub fn undo(&self) {
        match &*self.borrow() {
            FreeKind::UndoableLinked { previous, .. } => {
                let prev = *previous.clone();
                self.force_replace(prev);
            }
            _other => panic!("cannot undo"),
        }
    }

    pub fn unwrap_unbound(self) -> (Option<Str>, usize, Constraint) {
        match self.clone_inner() {
            FreeKind::Linked(_) | FreeKind::UndoableLinked { .. } => panic!("the value is linked"),
            FreeKind::Unbound {
                constraint, lev, ..
            } => (None, lev, constraint),
            FreeKind::NamedUnbound {
                name,
                lev,
                constraint,
            } => (Some(name), lev, constraint),
        }
    }

    pub fn unwrap_linked(self) -> T {
        match self.clone_inner() {
            FreeKind::Linked(t) | FreeKind::UndoableLinked { t, .. } => t,
            FreeKind::Unbound { .. } | FreeKind::NamedUnbound { .. } => {
                panic!("the value is unbounded")
            }
        }
    }

    pub fn detach(&self) -> Self {
        match self.clone().unwrap_unbound() {
            (Some(name), lev, constraint) => Self::new_named_unbound(name, lev, constraint),
            (None, lev, constraint) => Self::new_unbound(lev, constraint),
        }
    }
}

impl<T: CanbeFree> Free<T> {
    pub fn get_type(&self) -> Option<Type> {
        self.constraint().and_then(|c| c.get_type().cloned())
    }

    /// <: Super
    pub fn get_super(&self) -> Option<Type> {
        self.constraint().and_then(|c| c.get_super().cloned())
    }

    /// :> Sub
    pub fn get_sub(&self) -> Option<Type> {
        self.constraint().and_then(|c| c.get_sub().cloned())
    }

    pub fn get_subsup(&self) -> Option<(Type, Type)> {
        self.constraint()
            .and_then(|c| c.get_sub_sup().map(|(sub, sup)| (sub.clone(), sup.clone())))
    }

    pub fn is_unbound(&self) -> bool {
        matches!(
            &*self.borrow(),
            FreeKind::Unbound { .. } | FreeKind::NamedUnbound { .. }
        )
    }

    pub fn constraint_is_typeof(&self) -> bool {
        self.constraint()
            .map(|c| c.get_type().is_some())
            .unwrap_or(false)
    }

    pub fn constraint_is_sandwiched(&self) -> bool {
        self.constraint()
            .map(|c| c.get_sub_sup().is_some())
            .unwrap_or(false)
    }

    pub fn constraint_is_uninited(&self) -> bool {
        self.constraint().map(|c| c.is_uninited()).unwrap_or(false)
    }

    /// if `in_inst_or_gen` is true, constraint will be updated forcibly
    pub fn update_constraint(&self, new_constraint: Constraint, in_inst_or_gen: bool) {
        match unsafe { &mut *self.as_ptr() as &mut FreeKind<T> } {
            FreeKind::Unbound {
                lev, constraint, ..
            }
            | FreeKind::NamedUnbound {
                lev, constraint, ..
            } => {
                if !in_inst_or_gen && *lev == GENERIC_LEVEL {
                    log!(err "cannot update the constraint of a generalized type variable");
                    return;
                }
                if addr_eq!(*constraint, new_constraint) {
                    return;
                }
                *constraint = new_constraint;
            }
            FreeKind::Linked(t) | FreeKind::UndoableLinked { t, .. } => {
                t.update_constraint(new_constraint, in_inst_or_gen);
            }
        }
    }
}

impl Free<TyParam> {
    pub fn map<F>(&self, f: F)
    where
        F: Fn(TyParam) -> TyParam,
    {
        match unsafe { &mut *self.as_ptr() as &mut FreeKind<TyParam> } {
            FreeKind::Unbound { .. } | FreeKind::NamedUnbound { .. } => {
                panic!("the value is unbounded")
            }
            FreeKind::Linked(t) | FreeKind::UndoableLinked { t, .. } => {
                *t = f(mem::take(t));
            }
        }
    }
}

pub type FreeTyVar = Free<Type>;
pub type FreeTyParam = Free<TyParam>;

mod tests {
    #![allow(unused_imports)]
    use erg_common::enable_overflow_stacktrace;

    use crate::ty::constructors::*;
    use crate::ty::*;
    use crate::*;

    #[test]
    fn cmp_freevar() {
        enable_overflow_stacktrace!();
        let t = named_uninit_var("T".into());
        let Type::FreeVar(fv) = t.clone() else { unreachable!() };
        let constraint = Constraint::new_subtype_of(poly("Add", vec![ty_tp(t.clone())]));
        fv.update_constraint(constraint.clone(), true);
        let u = named_free_var("T".into(), 1, constraint);
        println!("{t} {u}");
        assert_eq!(t, t);
        assert_eq!(t, u);
    }
}
