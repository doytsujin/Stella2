//! Declarative UI for TCW3
//!
//! Most parts of UI are static and imperative programming is not the best
//! option to write such things as it leads to an excessive amount of
//! boilerplate code. TCW3 Designer is a code generation framework that
//! addresses this issue.
//!
//! TCW3 designer is designed to meet the following requirements:
//!
//! - The structures of UI components can be expressed in a way that is mostly
//!   free of boilerplate code for procedurally constructing a structure.
//! - It generates widget controller types akin to standard widgets such as
//!   `tcw3::ui::views::Button` and they can be used in a similar way.
//! - Components in one crate can consume other components from another crate.
//! - Seamlessly integrates with existing components.
//!
//! # Usage
//!
//! TODO - please see `tcw3_meta`.
//!
//! # Language Reference
//!
//! TODO
//!
//! ## Component Attributes
//!
//!  - **`#[prototype_only]`** suppresses the generation of implementation code.
//!  - **`#[widget]`** indicates that the component is a widget controller type.
//!    The precise semantics is yet to be defined and this attribute does
//!    nothing at the moment.
//!  - **`#[builder(simple)]`** changes the builder API to the simple one used
//!    by standard widgets. Because Designer does not support generating the
//!    code generation for the simple builder API, **`#[prototype_only]` must be
//!    also specified**.
//!
//!    The simple builder API does not provide a builder type and instead the
//!    component is instantiated by its method `new` that accepts initial field
//!    values in the order defined in the component. Optional `const` fields
//!    are not allowed to have a setter method because there's no way to set
//!    them. This means that every `const` field either (1) has no default value
//!    and must be specified through `new` or (2) has a default value that can't
//!    be changed from outside.
//!
//!    ```rust,no_compile
//!    // Standard builder
//!    ScrollbarBuilder::new().vertical(true).build()
//!    // Simple builder
//!    Scrollbar::new(true).build()
//!    ```
//!
//!    The reason to support this builder API is to facilitate the integration
//!    with hand-crafted components since the simple builder API is easier to
//!    write manually.
//!
//! ## Inputs
//!
//! *Inputs* (e.g., `this.prop` in `wire foo = |&this.prop| *prop + 42`)
//! represent a value used as an input to calculation as well as specifying
//! the trigger of an event handler. They are defined recursively as follows:
//!
//!  - `ϕ` is de-sugared into `this.ϕ` if it does not start with `this.` or
//!    `event.`.
//!  - `this` is an input.
//!  - `this.item` is an input if the enclosing component (the surrounding
//!    `comp` block) has a field or event named `field`.
//!  - If `ϕ` is an input representing a `const`¹ field, the field
//!    stores a component, and the said component has a field or event named
//!    `item`, then `ϕ.item` is an input.
//!  - `event.param` is an input if the input is specified in the handler
//!    function of an `on` item (i.e., in `on (x) |y| { ... }`, `y` meets this
//!    condition but `x` does not), the trigger input (i.e., `x` in the previous
//!    example) only contains inputs representing one or more events, and all of
//!    the said events have a parameter named `param`.
//!
//! ¹ This restriction may be lifted in the future.
//!
//! Inputs appear in various positions with varying roles, which impose
//! restrictions on the kinds of the inputs' referents:
//!
//! | Position              | Role     |
//! | --------------------- | -------- |
//! | `on` trigger          | Trigger  |
//! | `on` handler function | Sampled  |
//! | `const`               | Static   |
//! | `prop`                | Static   |
//! | `wire`                | Reactive |
//! | obj-init → `const`    | Static   |
//! | obj-init → `prop`     | Reactive |
//!
//! - The role is **Reactive** or **Trigger**, the input must be watchable. That
//!   is, the referent must be one of the following:
//!     - A `const` field.
//!     - A `prop` or `wire` field in a component other than the enclosing
//!       component, having a `watch` accessor visible to the enclosing
//!       component.
//!     - Any field of the enclosing component.
//!     - An `event` item.
//! - The role is **Static**, the referent must be a `const` field.
//!
//! ## Limiations
//!
//! - The code generator does not have access to Rust's full type system.
//!   Therefore, it does not perform type chacking at all.
//!
//! # Implementation Details
//!
//! ## Crate Metadata
//!
//! ```text
//! ,-> tcw3 -> tcw3_designer_runtime                    tcw3_designer <-,
//! |                                                                    |
//! |    ,----------,  dep   ,---------------,  codegen  ,----------,    |
//! | <- | upstream | -----> | upstream_meta | <-------- | build.rs | -> |
//! |    '----------'        '---------------'           '----------'    |
//! |         ^                      ^                         build-dep |
//! |         |                      |       build-dep                   |
//! |         | dep                  '------------------------,          |
//! |         |                                               |          |
//! |         |                                               |          |
//! |    ,----------,  dep   ,---------------,  codegen  ,----------,    |
//! '--- | applicat | -----> | applicat_meta | <-------- | build.rs | ---'
//!      '----------'        '---------------'           '----------'
//! ```
//!
//! In order to enable the consumption of other crate's components, TCW3
//! Designer makes use of build scripts. Each widget library crate has a meta
//! crate indicated by the suffix `_meta`. The source code of each meta crate
//! is generated by the build script, which can access other crates' information
//! by importing their meta crates through `build-dependencies`.
//!
//! ## Meta Crates
//!
//! Meta crates include a build script that uses [`BuildScriptConfig`] to
//! generate the source code of the crate. The generated code exports the
//! following two items:
//!
//! ```rust,no_compile
//! pub static DESIGNER_METADATA: &[u8] = [ /* ... */ ];
//! #[macro_export] macro_rules! designer_impl { /* ... */ }
//! ```
//!
//! `DESIGNER_METADATA` is encoded metadata, describing components and their
//! interfaces provided by the crate. You call [`BuildScriptConfig::link`] to
//! import `DESIGNER_METADATA` from another crate.
//!
//! `designer_impl` is used by the main crate to generate the skeleton
//! implementation for the defined components.
//!
//! ## Component Types
//!
//! For a `pub` component named `Component`, the following four types are
//! defined (they are inserted to a source file by `designer_impl` macro):
//!
//! ```rust,no_compile
//! pub struct ComponentBuilder<T_mandatory_attr> {
//!     mandatory_attr: T_mandatory_attr,
//!     optional_attr: Option<u32>,
//! }
//!
//! pub struct Component {
//!     shared: Rc<ComponentShared>,
//! }
//!
//! struct ComponentShared {
//!     state: RefCell<ComponentState>,
//!     value_const1: u32,
//!     subscriptions_event1: RefCell<_>,
//!     /* ... */
//! }
//!
//! struct ComponentState {
//!     value_prop1: u32,
//!     value_wire1: u32,
//!     /* ...*/
//! }
//! ```
//!
//! ## Builder Types
//!
//! `ComponentBuilder` implements a moving builder pattern (where methods take
//! `Self` and return a modified instance, destroying the original one). It
//! uses a generics-based trick to ensure that the mandatory parameters are
//! properly set at compile-time.
//!
//! ```rust,no_compile
//! use tcw3::designer_runtime::Unset;
//!
//! pub struct ComponentBuilder<T_mandatory_attr> {
//!     mandatory_attr: T_mandatory_attr,
//! }
//!
//! // `Unset` represents those "holes"
//! impl ComponentBuilder<Unset> { pub fn new() -> Self { /* ... */ } }
//!
//! // `build` appears only if these holes are filled
//! impl ComponentBuilder<u32> { pub fn build() -> Component { /* ... */ } }
//! ```
//!
//! ## Component Initialization
//!
//! **Field Initialization** —
//! The first step in the component constructor `Component::new` is to evaluate
//! the initial values of all fields and construct `ComponentState`,
//! `ComponentShared`, and then finally `Component`.
//!
//! A dependency graph is constructed. Each node represents one of the
//! following: (1) A field having a value, which is either an object
//! initialization literal `OtherComp { ... }` or a function `|dep| expr`.
//! (2) A `const` or `prop` field in an object initialization literal in
//! `Component`.
//! A topological order is found and the values are evaluated according to that.
//! Note that because none of the component's structs are available at this
//! point, **`this` cannot be used as an input to any of the fields** involved
//! here. Obviously, fields that are not initialized at this point cannot be
//! used as an input.
//!
//! **Events** —
//! Event handlers are hooked up to child objects. `on (obj.event)` and
//! `on (obj.prop)` explicitly create event handlers. Props and wires with
//! functions like `|obj.prop| expr` register automatically-generated event
//! handlers for observing changes in the input values.
//!
//! The registration functions return `tcw3::designer_runtime::Sub`.
//! They are automatically unsubscribed when `Component` is dropped.
//!
//! Event handlers maintain weak references to `ComponentShared`.
//!
//! ## Updating State
//!
//! After dependencies are updated, recalculation (called *a commit operation*)
//! of props and wires is scheduled using `tcw3::uicore::WmExt::invoke_on_update`.
//! Since it's possible to borrow the values of props and wires anytime, the
//! callback function of `invoke_on_update` is the only place where the values
//! can be mutated reliably (though this is not guaranteed, so runtime checks
//! are still necessary for safety).
//! Most importantly, even the effect of prop setters are deferred in this way.
//! New prop values are stored in a separate location until they are assigned
//! during a commit operation.
//!
//! A bit array is used as dirty flags for tracking which fields need to be
//! recalculated. Basically, each prop and wire with a functional value receives
//! a dirty flag. (TODO: Optimize dirty flag mapping and propagation)
//!
mod codegen;
mod metadata;

pub use self::codegen::BuildScriptConfig;
