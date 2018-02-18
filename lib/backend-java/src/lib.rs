#[macro_use]
extern crate genco;
#[macro_use]
extern crate log;
extern crate reproto_backend as backend;
extern crate reproto_core as core;
extern crate reproto_manifest as manifest;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate toml;

mod processor;
mod java_backend;
mod java_options;
mod java_file;
mod listeners;
mod java_field;
mod module;
mod codegen;
mod utils;

use backend::Environment;
use core::Context;
use core::errors::*;
use java_backend::JavaBackend;
use java_options::JavaOptions;
use listeners::{Configure, Listeners};
use manifest::{Lang, Manifest, NoModule, TryFromToml};
use std::path::Path;
use std::rc::Rc;
use utils::Utils;

pub const JAVA_CONTEXT: &str = "java";

#[derive(Default)]
pub struct JavaLang;

impl Lang for JavaLang {
    type Module = JavaModule;

    fn comment(input: &str) -> Option<String> {
        Some(format!("// {}", input))
    }
}

#[derive(Debug)]
pub enum JavaModule {
    Jackson,
    Lombok,
    Grpc,
    Builder,
    ConstructorProperties,
    Mutable,
    Nullable,
    OkHttp(module::OkHttpConfig),
}

impl TryFromToml for JavaModule {
    fn try_from_string(path: &Path, id: &str, value: String) -> Result<Self> {
        use self::JavaModule::*;

        let result = match id {
            "jackson" => Jackson,
            "lombok" => Lombok,
            "grpc" => Grpc,
            "builder" => Builder,
            "constructor_properties" => ConstructorProperties,
            "mutable" => Mutable,
            "nullable" => Nullable,
            "okhttp" => OkHttp(module::OkHttpConfig::default()),
            _ => return NoModule::illegal(path, id, value),
        };

        Ok(result)
    }

    fn try_from_value(path: &Path, id: &str, value: toml::Value) -> Result<Self> {
        use self::JavaModule::*;

        let result = match id {
            "jackson" => Jackson,
            "lombok" => Lombok,
            "grpc" => Grpc,
            "builder" => Builder,
            "constructor_properties" => ConstructorProperties,
            "mutable" => Mutable,
            "nullable" => Nullable,
            "okhttp" => OkHttp(value.try_into()?),
            _ => return NoModule::illegal(path, id, value),
        };

        Ok(result)
    }
}

fn setup_options(modules: Vec<JavaModule>, utils: &Rc<Utils>) -> JavaOptions {
    use self::JavaModule::*;

    let mut options = JavaOptions::new();

    for module in modules {
        let listener: Box<Listeners> = match module {
            Jackson => Box::new(module::Jackson),
            Lombok => Box::new(module::Lombok),
            Grpc => Box::new(module::Grpc),
            Builder => Box::new(module::Builder),
            ConstructorProperties => Box::new(module::ConstructorProperties),
            Mutable => Box::new(module::Mutable),
            Nullable => Box::new(module::Nullable),
            OkHttp(config) => Box::new(module::OkHttp::new(config)),
        };

        listener.configure(Configure {
            options: &mut options,
            utils: utils,
        })
    }

    options
}

pub fn compile(ctx: Rc<Context>, env: Environment, manifest: Manifest<JavaLang>) -> Result<()> {
    let env = Rc::new(env);
    let utils = Rc::new(Utils::new(&env));
    let options = setup_options(manifest.modules, &utils);
    let backend = JavaBackend::new(&env, &utils, options);

    let handle = ctx.filesystem(manifest.output.as_ref().map(AsRef::as_ref))?;

    backend.compile(handle.as_ref())
}
