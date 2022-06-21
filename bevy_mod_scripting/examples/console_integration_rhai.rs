use bevy::{ecs::event::Events, prelude::*};
use bevy_console::{AddConsoleCommand, ConsoleCommand, ConsolePlugin, PrintConsoleLine};
use bevy_mod_scripting::{
    events::PriorityEventWriter, APIProvider, AddScriptHost, AddScriptHostHandler, Recipients,
    RhaiContext, RhaiEvent, RhaiFile, RhaiScriptHost, Script, ScriptCollection,
    ScriptErrorEvent, ScriptingPlugin, ScriptError, AddScriptApiProvider, RhaiDocFragment,
};
use rhai::{FuncArgs, Engine};

/// custom Rhai API, world is provided as a usize (by the script this time), since
/// Rhai does not allow global/local variable access from a callback
#[derive(Default)]
pub struct RhaiAPI;

impl APIProvider for RhaiAPI {
    type Target = Engine;
    type DocTarget = RhaiDocFragment;

    fn attach_api(&mut self,engine: &mut Self::Target) -> Result<(),ScriptError> {
        // rhai allows us to decouple the api from the script context,
        // so here we do not have access to the script scope, but the advantage is that 
        // this single engine is shared with all of our scripts.
        // we can also set script wide settings here like this one for all our scripts.
        
        engine.set_max_expr_depths(0, 0);

        engine.register_fn("print_to_console", |shared_world: usize, msg: String| {
                let world: &mut World = unsafe { &mut *(shared_world as *mut World) };

                let mut events: Mut<Events<PrintConsoleLine>> = world.get_resource_mut().unwrap();
                events.send(PrintConsoleLine { line: msg });

                ()
            });

        engine.register_fn("entity_id", |entity: Entity| entity.id());

        Ok(())
    }
}


#[derive(Clone)]
pub struct RhaiEventArgs {}

impl FuncArgs for RhaiEventArgs {
    fn parse<ARGS: Extend<rhai::Dynamic>>(self, _args: &mut ARGS) {}
}

/// sends updates to script host which are then handled by the scripts
/// in the designated stage
pub fn trigger_on_update_rhai(mut w: PriorityEventWriter<RhaiEvent<RhaiEventArgs>>) {
    let event = RhaiEvent {
        hook_name: "on_update".to_string(),
        args: RhaiEventArgs {},
        recipients: Recipients::All,
    };

    w.send(event, 0);
}

pub fn forward_script_err_to_console(
    mut r: EventReader<ScriptErrorEvent>,
    mut w: EventWriter<PrintConsoleLine>,
) {
    for e in r.iter() {
        w.send(PrintConsoleLine {
            line: format!("ERROR:{}", e.err),
        });
    }
}

fn main() -> std::io::Result<()> {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins)
        .add_plugin(ScriptingPlugin)
        .add_plugin(ConsolePlugin)
        .add_startup_system(watch_assets)
        // register bevy_console commands
        .add_console_command::<RunScriptCmd, _, _>(run_script_cmd)
        .add_console_command::<DeleteScriptCmd, _, _>(delete_script_cmd)
        // choose and register the script hosts you want to use
        .add_script_host::<RhaiScriptHost<RhaiEventArgs>, _>(CoreStage::PostUpdate)
        .add_api_provider::<RhaiScriptHost<RhaiEventArgs>>(Box::new(RhaiAPI))
        .add_script_handler_stage::<RhaiScriptHost<RhaiEventArgs>, _, 0, 0>(
            CoreStage::PostUpdate,
        )
        // add your systems
        .add_system(trigger_on_update_rhai)
        .add_system(forward_script_err_to_console);

    // at runtime press '~' for console then type in help for command formats
    app.run();

    Ok(())
}

// we use bevy-debug-console to demonstrate how this can fit in in the runtime of a game
// note that using just the entity id instead of the full Entity has issues,
// but since we aren't despawning/spawning entities this works in our case
#[derive(ConsoleCommand)]
#[console_command(name = "run_script")]
///Runs a Lua script from the `assets/scripts` directory
pub struct RunScriptCmd {
    /// the relative path to the script, e.g.: `/hello.lua` for a script located in `assets/scripts/hello.lua`
    pub path: String,

    /// the entity id to attach this script to
    pub entity: Option<u32>,
}

pub fn run_script_cmd(
    mut log: ConsoleCommand<RunScriptCmd>,
    server: Res<AssetServer>,
    mut commands: Commands,
    mut existing_scripts: Query<&mut ScriptCollection<RhaiFile>>,
) {
    if let Some(RunScriptCmd { path, entity }) = log.take() {
        let handle = server.load::<RhaiFile, &str>(&format!("scripts/{}", &path));

        match entity {
            Some(e) => {
                if let Ok(mut scripts) = existing_scripts.get_mut(Entity::from_raw(e)) {
                    info!("Creating script: scripts/{} {:?}", &path, e);

                    scripts.scripts.push(Script::<RhaiFile>::new::<
                        RhaiScriptHost<RhaiEventArgs>,
                    >(path, handle));
                } else {
                    log.reply_failed(format!("Something went wrong"));
                };
            }
            None => {
                info!("Creating script: scripts/{}", &path);

                commands.spawn().insert(ScriptCollection::<RhaiFile> {
                    scripts: vec![Script::<RhaiFile>::new::<
                        RhaiScriptHost<RhaiEventArgs>,
                    >(path, handle)],
                });
            }
        };
    }
}

/// optional, hot reloading
fn watch_assets(server: Res<AssetServer>) {
    server.watch_for_changes().unwrap();
}

pub fn delete_script_cmd(
    mut log: ConsoleCommand<DeleteScriptCmd>,
    mut scripts: Query<(Entity, &mut ScriptCollection<RhaiFile>)>,
) {
    if let Some(DeleteScriptCmd { name, entity_id }) = log.take() {
        for (e, mut s) in scripts.iter_mut() {
            if e.id() == entity_id {
                let old_len = s.scripts.len();
                s.scripts.retain(|s| s.name() != name);

                if old_len > s.scripts.len() {
                    log.reply_ok(format!("Deleted script {}, on entity: {}", name, entity_id));
                } else {
                    log.reply_failed(format!(
                        "Entity {} did own a script named: {}",
                        entity_id, name
                    ))
                };
                return;
            }
        }

        log.reply_failed("Could not find given entity ID with a script")
    }
}

#[derive(ConsoleCommand)]
#[console_command(name = "delete_script")]
///Runs a Lua script from the `assets/scripts` directory
pub struct DeleteScriptCmd {
    /// the name of the script
    pub name: String,

    /// the entity the script is attached to
    pub entity_id: u32,
}
