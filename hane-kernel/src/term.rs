use std::cmp::Ordering;
use std::fmt::{self, Display, Formatter};

use crate::entry::{Binder, Entry, EntryRef};
use crate::global::GEntryRef;
use crate::{Global, Sort, Stack, TypeError, TypeErrorVariant};

#[derive(Clone)]
pub struct Term<M, B> {
    pub meta: M,
    pub variant: Box<TermVariant<M, B>>,
}

#[derive(Clone)]
pub enum TermVariant<M, B> {
    Sort(Sort),
    Var(usize),
    Const(String),
    App(Term<M, B>, Term<M, B>),
    Product(B, Term<M, B>, Term<M, B>),
    Abstract(B, Term<M, B>, Term<M, B>),
    Bind(B, Term<M, B>, Term<M, B>, Term<M, B>),
    Match(Term<M, B>, B, MatchArm<M, B>, Vec<MatchArm<M, B>>),
}

#[derive(Clone)]
pub struct MatchArm<M, B> {
    pub constructor: String,
    pub params: Vec<B>,
    pub body: Term<M, B>,
}

impl<M, B> PartialEq for Term<M, B> {
    fn eq(&self, other: &Self) -> bool {
        self.variant == other.variant
    }
}

impl<M, B> PartialEq for TermVariant<M, B> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Sort(l0), Self::Sort(r0)) => l0 == r0,
            (Self::Var(l0), Self::Var(r0)) => l0 == r0,
            (Self::Const(l0), Self::Const(r0)) => l0 == r0,
            (Self::App(l0, l1), Self::App(r0, r1)) => l0 == r0 && l1 == r1,
            (Self::Product(_, l0, l1), Self::Product(_, r0, r1)) => l0 == r0 && l1 == r1,
            (Self::Abstract(_, l0, l1), Self::Abstract(_, r0, r1)) => l0 == r0 && l1 == r1,
            (Self::Bind(_, l0, l1, l2), Self::Bind(_, r0, r1, r2)) => {
                l0 == r0 && l1 == r1 && l2 == r2
            }
            _ => false,
        }
    }
}

impl<M, B> Eq for Term<M, B> {}
impl<M, B> Eq for TermVariant<M, B> {}

impl<M, B> Display for TermVariant<M, B> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            TermVariant::Sort(sort) => write!(f, "{sort}"),
            TermVariant::Var(n) => write!(f, "'{}", n),
            TermVariant::Const(name) => write!(f, "{}", name),
            TermVariant::App(t1, t2) => write!(f, "({}) ({})", t1, t2),
            TermVariant::Product(_, t1, t2) => write!(f, "forall[{}] ({})", t1, t2),
            TermVariant::Abstract(_, t1, t2) => write!(f, "fun[{}] ({})", t1, t2),
            TermVariant::Bind(_, t1, t2, t3) => write!(f, "let[{} : {}] ({})", t1, t2, t3),
            TermVariant::Match(t, _, ret, arms) => {
                write!(
                    f,
                    "match {t} in {} return {} with",
                    ret.constructor, ret.body
                )?;
                let mut sep = "";
                for arm in arms {
                    write!(f, "{sep} {} => {}", arm.constructor, arm.body)?;
                    sep = " |";
                }
                write!(f, " end")
            }
        }
    }
}

impl<M, B> Display for Term<M, B> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.variant)
    }
}

impl<M: Clone, B: Clone> TermVariant<M, B> {
    fn push_inner(&self, cut: usize, amount: usize) -> Self {
        match self {
            TermVariant::Sort(sort) => TermVariant::Sort(sort.clone()),
            TermVariant::Var(n) => TermVariant::Var(if cut <= *n { *n + amount } else { *n }),
            TermVariant::Const(name) => TermVariant::Const(name.to_owned()),
            TermVariant::App(f, v) => {
                TermVariant::App(f.push_inner(cut, amount), v.push_inner(cut, amount))
            }
            TermVariant::Product(x, x_tp, t) => TermVariant::Product(
                x.clone(),
                x_tp.push_inner(cut, amount),
                t.push_inner(cut + 1, amount),
            ),
            TermVariant::Abstract(x, x_tp, t) => TermVariant::Abstract(
                x.clone(),
                x_tp.push_inner(cut, amount),
                t.push_inner(cut + 1, amount),
            ),
            TermVariant::Bind(x, x_tp, x_val, t) => TermVariant::Bind(
                x.clone(),
                x_tp.push_inner(cut, amount),
                x_val.push_inner(cut, amount),
                t.push_inner(cut + 1, amount),
            ),
            TermVariant::Match(_, _, _, _) => todo!(),
        }
    }
}

impl<M: Clone, B: Clone> Term<M, B> {
    //TODO: Find a more descriptive name
    pub fn push(&self, amount: usize) -> Self {
        self.push_inner(0, amount)
    }

    fn push_inner(&self, cut: usize, amount: usize) -> Self {
        Term {
            meta: self.meta.clone(),
            variant: Box::new(self.variant.push_inner(cut, amount)),
        }
    }

    pub fn subst(&self, mut f: impl FnMut(&M, usize, usize) -> Self) -> Self {
        self.subst_inner(0, &mut f)
    }

    fn subst_inner(&self, push: usize, f: &mut impl FnMut(&M, usize, usize) -> Self) -> Self {
        let variant = match &*self.variant {
            TermVariant::Sort(sort) => TermVariant::Sort(sort.clone()),
            TermVariant::Var(n) => return f(&self.meta, *n, push),
            TermVariant::Const(name) => TermVariant::Const(name.to_owned()),
            TermVariant::App(t, v) => {
                TermVariant::App(t.subst_inner(push, f), v.subst_inner(push, f))
            }
            TermVariant::Product(x, x_tp, t) => TermVariant::Product(
                x.clone(),
                x_tp.subst_inner(push, f),
                t.subst_inner(push + 1, f),
            ),
            TermVariant::Abstract(x, x_tp, t) => TermVariant::Abstract(
                x.clone(),
                x_tp.subst_inner(push, f),
                t.subst_inner(push + 1, f),
            ),
            TermVariant::Bind(x, x_tp, x_val, t) => TermVariant::Bind(
                x.clone(),
                x_tp.subst_inner(push, f),
                x_val.subst_inner(push, f),
                t.subst_inner(push + 1, f),
            ),
            TermVariant::Match(_, _, _, _) => todo!(),
        };
        Term {
            meta: self.meta.clone(),
            variant: Box::new(variant),
        }
    }

    pub fn try_subst<E>(
        &self,
        mut f: impl FnMut(&M, usize, usize) -> Result<Self, E>,
    ) -> Result<Self, E> {
        self.try_subst_inner(0, &mut f)
    }

    fn try_subst_inner<E>(
        &self,
        push: usize,
        f: &mut impl FnMut(&M, usize, usize) -> Result<Self, E>,
    ) -> Result<Self, E> {
        let variant = match &*self.variant {
            TermVariant::Sort(sort) => TermVariant::Sort(sort.clone()),
            TermVariant::Var(n) => return f(&self.meta, *n, push),
            TermVariant::Const(name) => TermVariant::Const(name.to_owned()),
            TermVariant::App(t, v) => {
                TermVariant::App(t.try_subst_inner(push, f)?, v.try_subst_inner(push, f)?)
            }
            TermVariant::Product(x, x_tp, t) => TermVariant::Product(
                x.clone(),
                x_tp.try_subst_inner(push, f)?,
                t.try_subst_inner(push + 1, f)?,
            ),
            TermVariant::Abstract(x, x_tp, t) => TermVariant::Abstract(
                x.clone(),
                x_tp.try_subst_inner(push, f)?,
                t.try_subst_inner(push + 1, f)?,
            ),
            TermVariant::Bind(x, x_tp, x_val, t) => TermVariant::Bind(
                x.clone(),
                x_tp.try_subst_inner(push, f)?,
                x_val.try_subst_inner(push, f)?,
                t.try_subst_inner(push + 1, f)?,
            ),
            TermVariant::Match(_, _, _, _) => todo!(),
        };
        Ok(Term {
            meta: self.meta.clone(),
            variant: Box::new(variant),
        })
    }

    pub fn subst_single(&self, n: usize, val: &Self) -> Self {
        self.subst(|meta, x, push| match (n + push).cmp(&x) {
            Ordering::Less => Term {
                meta: meta.clone(),
                variant: Box::new(TermVariant::Var(x - 1)),
            },
            Ordering::Equal => val.push(push),
            Ordering::Greater => Term {
                meta: meta.clone(),
                variant: Box::new(TermVariant::Var(x)),
            },
        })
    }

    pub fn pop(&self, n: usize) -> Option<Self> {
        self.try_subst(|meta, x, push| match (n + push).cmp(&x) {
            Ordering::Less => Ok(Term {
                meta: meta.clone(),
                variant: Box::new(TermVariant::Var(x - 1)),
            }),
            Ordering::Equal => Err(()),
            Ordering::Greater => Ok(Term {
                meta: meta.clone(),
                variant: Box::new(TermVariant::Var(x)),
            }),
        })
        .ok()
    }

    pub fn subst_many<'a>(
        &'a self,
        n: usize,
        len: usize,
        vals: impl Fn(usize) -> &'a Self,
    ) -> Self {
        self.subst(|meta, x, push| {
            if x < n + push {
                Term {
                    meta: meta.clone(),
                    variant: Box::new(TermVariant::Var(x)),
                }
            } else if x < n + push + len {
                let i = n + push + len - 1 - x;
                vals(i).push(push)
            } else {
                Term {
                    meta: meta.clone(),
                    variant: Box::new(TermVariant::Var(x - len)),
                }
            }
        })
    }

    pub fn normalize(&mut self, global: &Global<M, B>, local: &mut Stack<Entry<M, B>>) {
        loop {
            match &mut *self.variant {
                TermVariant::Sort(_) => break,
                TermVariant::Var(n) => {
                    // δ reduction
                    if let Some(value) = &local.get(*n).unwrap().value {
                        // To move the value into scope, it must first be pushed passed it self, then passed the other `n`
                        *self = value.push(*n + 1);
                        continue;
                    }
                }
                TermVariant::Const(name) => {
                    // δ reduction
                    if let Some(value) = global.get(name).unwrap().value {
                        *self = value.push(local.len());
                        continue;
                    }
                }
                TermVariant::App(f, v) => {
                    f.normalize(global, local);
                    v.normalize(global, local);

                    // β reduction
                    if let TermVariant::Abstract(_, _, t) = &*f.variant {
                        *self = t.subst_single(0, v);
                        continue;
                    }
                }
                TermVariant::Product(x, input_type, output_type) => {
                    input_type.normalize(global, local);
                    let mut local = local.push(Entry::new(x.clone(), input_type.clone()));
                    output_type.normalize(global, &mut local);
                }
                TermVariant::Abstract(x, input_type, body) => {
                    input_type.normalize(global, local);
                    let mut local = local.push(Entry::new(x.clone(), input_type.clone()));
                    body.normalize(global, &mut local);
                }
                TermVariant::Bind(_name, _type, val, t) => {
                    val.normalize(global, local);
                    // ζ reduction (Remove let binding)
                    *self = t.subst_single(0, val);
                    continue;
                }
                TermVariant::Match(_, _, _, _) => todo!(),
            }
            break;
        }
    }

    fn eta(&mut self) {
        match &mut *self.variant {
            TermVariant::Sort(_) => {}
            TermVariant::Var(_) => {}
            TermVariant::Const(_) => {}
            TermVariant::App(f, v) => {
                f.eta();
                v.eta();
            }
            TermVariant::Product(_, input_type, output_type) => {
                input_type.eta();
                output_type.eta();
            }
            TermVariant::Abstract(_, input_type, body) => {
                input_type.eta();
                body.eta();
            }
            TermVariant::Bind(_, _, _, _) => unreachable!(),
            TermVariant::Match(_, _, _, _) => todo!(),
        }

        if let TermVariant::Abstract(_, _, body) = &*self.variant {
            if let TermVariant::App(f, v) = &*body.variant {
                if let TermVariant::Var(0) = &*v.variant {
                    if let Some(f) = f.pop(0) {
                        *self = f;
                    }
                }
            }
        }
    }

    pub fn expect_convertable(
        &self,
        other: &Self,
        global: &Global<M, B>,
        local: &mut Stack<Entry<M, B>>,
    ) -> Result<(), TypeError<M, B>> {
        let mut this = self.clone();
        let mut other = other.clone();
        this.normalize(global, local);
        this.eta();
        other.normalize(global, local);
        other.eta();
        if this == other {
            Ok(())
        } else {
            Err(TypeError::new(
                local,
                TypeErrorVariant::IncompatibleTypes(other.clone(), self.clone()),
            ))
        }
    }

    fn subtype_inner(&self, other: &Self, global: &Global<M, B>) -> bool {
        match (&*self.variant, &*other.variant) {
            (TermVariant::Sort(l), TermVariant::Sort(r)) => l <= r,
            (TermVariant::Product(_, l0, l1), TermVariant::Product(_, r0, r1)) => {
                l0 == r0 && l1.subtype_inner(r1, global)
            }
            (l, r) => l == r,
        }
    }

    pub fn expect_subtype(
        &self,
        other: &Self,
        global: &Global<M, B>,
        local: &mut Stack<Entry<M, B>>,
    ) -> Result<(), TypeError<M, B>> {
        let mut this = self.clone();
        let mut other0 = other.clone();
        this.normalize(global, local);
        this.eta();
        other0.normalize(global, local);
        other0.eta();
        if this.subtype_inner(&other0, global) {
            Ok(())
        } else {
            Err(TypeError::new(
                local,
                TypeErrorVariant::NotSubtypeType(other.clone(), self.clone()),
            ))
        }
    }

    /// Seperates terms of the form `forall (x1 : T1) .. (xn : Tn), t` into `([(x1 : T1), .. , (xn : Tn)], t)`.
    /// If the input is not a product, it is returned unchanged.
    pub fn strip_products(mut self) -> (Vec<Binder<M, B>>, Self) {
        let mut arity = Vec::new();
        while let TermVariant::Product(x, ttype, body) = *self.variant {
            arity.push(Binder { x, ttype });
            self = body
        }
        (arity, self)
    }

    /// Seperates terms of the form `f v1 .. vn` into `(f, [v1, .. , vn])`.
    /// If the input is not an application, it is returned unchanged.
    pub fn strip_args(mut self) -> (Self, Vec<Self>) {
        let mut args = Vec::new();
        while let TermVariant::App(fun, arg) = *self.variant {
            args.push(arg);
            self = fun
        }
        args.reverse();
        (self, args)
    }

    pub fn expect_sort(
        &self,
        global: &Global<M, B>,
        local: &mut Stack<Entry<M, B>>,
    ) -> Result<Sort, TypeError<M, B>> {
        let mut t = self.clone();
        t.normalize(global, local);
        if let TermVariant::Sort(sort) = *t.variant {
            Ok(sort)
        } else {
            Err(TypeError::new(
                local,
                TypeErrorVariant::NotASort(self.clone()),
            ))
        }
    }

    pub fn expect_product(
        mut self,
        global: &Global<M, B>,
        local: &mut Stack<Entry<M, B>>,
    ) -> Result<(Self, Self), TypeError<M, B>> {
        self.normalize(global, local);
        if let TermVariant::Product(_, input_type, output_type) = *self.variant {
            Ok((input_type, output_type))
        } else {
            Err(TypeError::new(local, TypeErrorVariant::NotAProduct(self)))
        }
    }

    pub fn type_check(
        &self,
        global: &Global<M, B>,
        local: &mut Stack<Entry<M, B>>,
    ) -> Result<Self, (M, TypeError<M, B>)> {
        Ok(match &*self.variant {
            TermVariant::Sort(sort) => Term {
                meta: self.meta.clone(),
                variant: Box::new(TermVariant::Sort(sort.ttype())),
            },
            TermVariant::Var(n) => {
                // To move the type into scope, it must first be pushed passed it self, then passed the other `n`
                return local.get(*n).map(|e| e.ttype.push(*n + 1)).ok_or_else(|| {
                    (
                        self.meta.clone(),
                        TypeError::new(local, TypeErrorVariant::DebruijnOutOfScope(*n)),
                    )
                });
            }
            TermVariant::Const(name) => {
                return global
                    .get(name)
                    .map(|EntryRef { ttype, .. }| ttype.push(local.len()))
                    .ok_or_else(|| {
                        (
                            self.meta.clone(),
                            TypeError::new(local, TypeErrorVariant::UndefinedConst(name.clone())),
                        )
                    })
            }
            TermVariant::App(f, v) => {
                let f_tp = f.type_check(global, local)?;
                let (input_type, output_type) = f_tp
                    .expect_product(global, local)
                    .map_err(|err| (f.meta.clone(), err))?;
                let v_tp = v.type_check(global, local)?;
                v_tp.expect_subtype(&input_type, global, local)
                    .map_err(|err| (self.meta.clone(), err))?;
                output_type.subst_single(0, v)
            }
            TermVariant::Product(x, x_tp, t) => {
                let x_sort = x_tp.type_check(global, local)?;
                let x_sort = x_sort
                    .expect_sort(global, local)
                    .map_err(|err| (x_tp.meta.clone(), err))?;
                let mut local = local.push(Entry::new(x.clone(), x_tp.clone()));
                let t_tp = t.type_check(global, &mut local)?;
                let t_sort = t_tp
                    .expect_sort(global, &mut local)
                    .map_err(|err| (t.meta.clone(), err))?;
                Term {
                    meta: self.meta.clone(),
                    variant: Box::new(TermVariant::Sort(x_sort.product(t_sort))),
                }
            }
            TermVariant::Abstract(x, x_tp, t) => {
                let x_sort = x_tp.type_check(global, local)?;
                x_sort
                    .expect_sort(global, local)
                    .map_err(|err| (x_tp.meta.clone(), err))?;
                let mut local = local.push(Entry::new(x.clone(), x_tp.clone()));
                let t_tp = t.type_check(global, &mut local)?;
                Term {
                    meta: self.meta.clone(),
                    variant: Box::new(TermVariant::Product(x.clone(), x_tp.clone(), t_tp)),
                }
            }
            TermVariant::Bind(x, x_tp, x_val, t) => {
                let x_sort = x_tp.type_check(global, local)?;
                x_sort
                    .expect_sort(global, local)
                    .map_err(|err| (x_tp.meta.clone(), err))?;
                let x_val_tp = x_val.type_check(global, local)?;
                x_val_tp
                    .expect_subtype(x_tp, global, local)
                    .map_err(|err| (x_val.meta.clone(), err))?;
                let t_subst = t.subst_single(0, x_val);
                let mut local = local.push(Entry::new(x.clone(), x_tp.clone()));
                t_subst.type_check(global, &mut local)?
            }
            TermVariant::Match(t, name, ret, arms) => {
                let (i, params, bodies) = match global.get_entry(&ret.constructor) {
                    Some(GEntryRef::Inductive(i, params, bodies)) => (i, params, bodies),
                    Some(_) => todo!(),
                    None => todo!(),
                };
                let body = &bodies[i];
                // Insure the parameter count on the return pattern is correct
                if ret.params.len() != params.len() + body.arity.len() {
                    todo!()
                }
                let mut t_type = t.type_check(global, local)?;
                t_type.normalize(global, local);
                let (hd, mut args) = t_type.strip_args();
                if let TermVariant::Const(i) = *hd.variant {
                    if i != ret.constructor {
                        todo!()
                    }
                } else {
                    todo!()
                };
                let arity_args = args.drain(params.len()..).collect::<Vec<_>>();

                {
                    let mut local = local.slot();
                    local.extend(ret.params.iter().zip(params).zip(&args).enumerate().map(
                        |(i, ((x, param), value))| {
                            Entry::with_value(x.clone(), value.push(i), param.ttype.clone())
                        },
                    ));
                    local.extend(
                        ret.params[params.len()..]
                            .iter()
                            .zip(&body.arity)
                            .map(|(x, param)| Entry::new(x.clone(), param.ttype.clone())),
                    );
                    let ttype = (0..ret.params.len())
                        .rev()
                        .map(|n| Term {
                            meta: self.meta.clone(),
                            variant: Box::new(TermVariant::Var(n)),
                        })
                        .fold(
                            Term {
                                meta: self.meta.clone(),
                                variant: Box::new(TermVariant::Const(ret.constructor.clone())),
                            },
                            |f, v| Term {
                                meta: self.meta.clone(),
                                variant: Box::new(TermVariant::App(f, v)),
                            },
                        );
                    local.push_onto(Entry::new(name.clone(), ttype));
                    let ret_sort = ret.body.type_check(global, &mut local)?;
                    let sort = ret_sort
                        .expect_sort(global, &mut local)
                        .map_err(|err| (self.meta.clone(), err))?;

                    if sort == Sort::Prop && body.sort != Sort::Prop && body.constructors.len() > 1
                    {
                        todo!()
                    }
                };

                let mut constrs = vec![false; body.constructors.len()];
                for arm in arms {
                    let constructor = if let Some((i, constructor)) = body
                        .constructors
                        .iter()
                        .enumerate()
                        .find(|(_, c)| c.name == arm.constructor)
                    {
                        if constrs[i] {
                            todo!()
                        }
                        constrs[i] = true;
                        constructor
                    } else {
                        todo!()
                    };

                    if arm.params.len() != params.len() + constructor.arity.len() {
                        todo!()
                    }
                    let mut local = local.slot();
                    local.extend(arm.params.iter().zip(params).zip(&args).enumerate().map(
                        |(i, ((x, param), value))| {
                            Entry::with_value(x.clone(), value.push(i), param.ttype.clone())
                        },
                    ));
                    local.extend(
                        arm.params[params.len()..]
                            .iter()
                            .zip(&constructor.arity)
                            .map(|(x, param)| Entry::new(x.clone(), param.ttype.clone())),
                    );
                    let arm_type = arm.body.type_check(global, &mut local)?;
                    let exp_type = ret.body.push(arm.params.len()).subst_many(
                        arm.params.len(),
                        constructor.args.len() + 1,
                        |i| {
                            if i < constructor.args.len() {
                                &constructor.args[i]
                            } else {
                                &constructor.ttype
                            }
                        },
                    );
                    arm_type
                        .expect_subtype(&exp_type, global, &mut local)
                        .map_err(|err| (arm.body.meta.clone(), err))?;
                }

                if !constrs.into_iter().all(|b| b) {
                    todo!()
                }

                ret.body
                    .subst_many(0, args.len() + arity_args.len() + 1, |i| {
                        if i < args.len() {
                            &args[i]
                        } else if i < args.len() + arity_args.len() {
                            &arity_args[i - args.len()]
                        } else {
                            t
                        }
                    })
            }
        })
    }
}
