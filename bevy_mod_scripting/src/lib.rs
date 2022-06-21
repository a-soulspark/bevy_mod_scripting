#![doc=include_str!("../../readme.md")]

use std::{env, process};

use bevy::{ecs::schedule::IntoRunCriteria, prelude::*};

pub mod error;
pub mod hosts;

pub use bevy_event_priority as events;
pub use {error::*, hosts::*};

#[derive(Default)]
/// Bevy plugin enabling run-time scripting
pub struct ScriptingPlugin;

impl Plugin for ScriptingPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_event::<ScriptErrorEvent>();
    }
}

/// An error coming from a script
#[derive(Debug)]
pub struct ScriptErrorEvent {
    pub err: ScriptError,
}



pub trait GenDocumentation {
    fn gen_documentation<T : ScriptHost>(&mut self) -> &mut Self;
}

impl GenDocumentation for App {
    /// generates documentation if it detects the environment variable `GEN_SCRIPT_DOC` is set
    /// exits the process. Otherwise this is a no-op
    fn gen_documentation<T : ScriptHost>(&mut self) -> &mut Self {

        if env::var("GEN_SCRIPT_DOC").is_ok(){
            info!("Generating documentation");
            let w = &mut self.world;
            let providers : APIProviders<T::APITarget,T::DocTarget> = w.remove_resource().expect("No APIProviders resource found for the given script host. Did you forget to register the host?");
            providers.gen_all().expect("Could not generate documentation");
            println!("Documentation generated successfully, exiting..");
            process::exit(0);
        }


        self
    }
}


/// Trait for app builder notation
pub trait AddScriptHost {
    /// registers the given script host with your app
    fn add_script_host<T: ScriptHost, S: StageLabel>(&mut self, stage: S) -> &mut Self;
}

impl AddScriptHost for App {
    fn add_script_host<T: ScriptHost, S: StageLabel>(&mut self, stage: S) -> &mut Self {
        T::register_with_app(self, stage);
        self.init_resource::<T>();
        self
    }
}

pub trait AddScriptApiProvider {
    fn add_api_provider<T: ScriptHost>(&mut self, provider: Box<dyn APIProvider<Target=T::APITarget,DocTarget=T::DocTarget>>) -> &mut Self;
}

impl AddScriptApiProvider for App {
    fn add_api_provider<T : ScriptHost>(&mut self,provider: Box<dyn APIProvider<Target=T::APITarget,DocTarget=T::DocTarget>>) -> &mut Self {
        let w = &mut self.world;
        let providers : &mut APIProviders<T::APITarget,T::DocTarget> = &mut w.resource_mut();
        providers.providers.push(provider);
        self
    }
}

pub trait AddScriptHostHandler {
    /// Enables this script host to handle events with priorities in the range [0,min_prio] (inclusive),
    /// during the runtime of the given stage.
    ///
    /// Think of handler stages as a way to run certain types of events at various points in your engine.
    /// A good example of this is Unity [game loop's](https://docs.unity3d.com/Manual/ExecutionOrder.html) `onUpdate` and `onFixedUpdate`.
    /// FixedUpdate runs *before* any physics while Update runs after physics and input events.
    ///
    /// A similar setup can be achieved by using a separate stage before and after your physics,
    /// then assigning event priorities such that your events are forced to run at a particular stage, for example:
    ///
    /// PrePhysics: min_prio = 1
    /// PostPhysics: min_prio = 4
    ///
    /// | Priority | Handler     | Event        |
    /// | -------- | ----------- | ------------ |
    /// | 0        | PrePhysics  | Start        |
    /// | 1        | PrePhysics  | FixedUpdate  |
    /// | 2        | PostPhysics | OnCollision  |
    /// | 3        | PostPhysics | OnMouse      |
    /// | 4        | PostPhysics | Update       |
    ///
    /// The *frequency* of running these events, is controlled by your systems, if the event is not emitted, it cannot not handled.
    /// Of course there is nothing stopping your from emitting a single event type at varying priorities.
    fn add_script_handler_stage<T: ScriptHost, S: StageLabel, const MAX: u32, const MIN: u32>(
        &mut self,
        stage: S,
    ) -> &mut Self;

    /// Like `add_script_handler_stage` but with additional run criteria
    fn add_script_handler_stage_with_criteria<
        T: ScriptHost,
        S: StageLabel,
        M,
        C: IntoRunCriteria<M>,
        const MAX: u32,
        const MIN: u32,
    >(
        &mut self,
        stage: S,
        criteria: C,
    ) -> &mut Self;
}

impl AddScriptHostHandler for App {
    fn add_script_handler_stage<T: ScriptHost, S: StageLabel, const MAX: u32, const MIN: u32>(
        &mut self,
        stage: S,
    ) -> &mut Self {
        self.add_system_to_stage(
            stage,
            script_event_handler::<T, MAX, MIN>
                .exclusive_system()
                .at_end(),
        );
        self
    }

    fn add_script_handler_stage_with_criteria<
        T: ScriptHost,
        S: StageLabel,
        M,
        C: IntoRunCriteria<M>,
        const MAX: u32,
        const MIN: u32,
    >(
        &mut self,
        stage: S,
        criteria: C,
    ) -> &mut Self {
        self.add_system_to_stage(
            stage,
            script_event_handler::<T, MAX, MIN>
                .exclusive_system()
                .at_end()
                .with_run_criteria(criteria),
        );
        self
    }
}
