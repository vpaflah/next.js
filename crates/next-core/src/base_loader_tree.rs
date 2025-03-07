use anyhow::Result;
use indexmap::IndexMap;
use indoc::formatdoc;
use turbo_tasks::{RcStr, Value, ValueToString, Vc};
use turbo_tasks_fs::FileSystemPath;
use turbopack::{transition::Transition, ModuleAssetContext};
use turbopack_core::{
    file_source::FileSource,
    module::Module,
    reference_type::{EcmaScriptModulesReferenceSubType, ReferenceType},
};
use turbopack_ecmascript::{magic_identifier, utils::StringifyJs};

pub struct BaseLoaderTreeBuilder {
    pub inner_assets: IndexMap<RcStr, Vc<Box<dyn Module>>>,
    counter: usize,
    pub imports: Vec<RcStr>,
    pub module_asset_context: Vc<ModuleAssetContext>,
    pub server_component_transition: Vc<Box<dyn Transition>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComponentType {
    Page,
    DefaultPage,
    Error,
    Layout,
    Loading,
    Template,
    NotFound,
}

impl ComponentType {
    pub fn name(&self) -> &'static str {
        match self {
            ComponentType::Page => "page",
            ComponentType::DefaultPage => "defaultPage",
            ComponentType::Error => "error",
            ComponentType::Layout => "layout",
            ComponentType::Loading => "loading",
            ComponentType::Template => "template",
            ComponentType::NotFound => "not-found",
        }
    }
}

impl BaseLoaderTreeBuilder {
    pub fn new(
        module_asset_context: Vc<ModuleAssetContext>,
        server_component_transition: Vc<Box<dyn Transition>>,
    ) -> Self {
        BaseLoaderTreeBuilder {
            inner_assets: IndexMap::new(),
            counter: 0,
            imports: Vec::new(),
            module_asset_context,
            server_component_transition,
        }
    }

    pub fn unique_number(&mut self) -> usize {
        let i = self.counter;
        self.counter += 1;
        i
    }

    pub fn process_module(&self, path: Vc<FileSystemPath>) -> Vc<Box<dyn Module>> {
        let source = Vc::upcast(FileSource::new(path));

        let reference_type = Value::new(ReferenceType::EcmaScriptModules(
            EcmaScriptModulesReferenceSubType::Undefined,
        ));

        self.server_component_transition
            .process(source, self.module_asset_context, reference_type)
            .module()
    }

    pub async fn create_component_tuple_code(
        &mut self,
        component_type: ComponentType,
        path: Vc<FileSystemPath>,
    ) -> Result<String> {
        let name = component_type.name();
        let i = self.unique_number();
        let identifier = magic_identifier::mangle(&format!("{name} #{i}"));

        self.imports.push(
            formatdoc!(
                r#"
                import * as {} from "COMPONENT_{}";
                "#,
                identifier,
                i
            )
            .into(),
        );

        let module = self.process_module(path);

        self.inner_assets
            .insert(format!("COMPONENT_{i}").into(), module);

        let module_path = module.ident().path().to_string().await?;

        Ok(format!(
            "[() => {identifier}, {path}]",
            path = StringifyJs(&module_path),
        ))
    }
}
