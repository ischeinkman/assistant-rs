# AssistantRS

AssistantRS is a simple, configurable, and fully offline voice control program powered by [Mozilla DeepSpeech](https://github.com/mozilla/DeepSpeech) and Rust. It can either be run as a one-off to run a single command, or can be run in the background for continuous assistance without having to reload constantly.


## Usage

The program accepts the following flags:

*  `--config <file path>` -- Read configuration from `<file path>`. Can be passed more than once to read multiple config files. See [Configuration](#Configuration) for more information.
*  `--daemonize` or `-d` -- Run in "daemon" mode. 
*  `--help` or `-h`  -- Outputs usage information and exits. 
*  `--version` or `-V` -- Outputs version information and exits.

By default, the program loads the configuration and model, listens for audio until it no longer detects human speech, runs the command closest to the detected message, and exits.
When the `-d` flag is passed, the program loads the config and then sleeps until it recieves a Unix signal before responding as follows:

*  `SIGCONT` | `SIGUSR1` -- The program wakes up, listens and spawns a single command (following the same process as the default standalone mode), and sleeps again.
*  `SIGHUP` -- The program wakes up, re-reads the config files (including the ones previously passed via the `--config` flag), and reloads the model if necessary. 



## Configuration

Configuration is done via `toml` files. The following fields are defined:

| Field Name     | Type                                     | Description                                                                                                                                                                                                                    | Required? | Default                                                   |
| -------------- | ---------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------- | --------------------------------------------------------- |
| `library-path` | String                                   | The path to the `libdeepspeech.so` library file to use.                                                                                                                                                                        | No        | None; `libdeepspeech.so` is loaded via the system loader. |
| `model-path`   | String                                   | The path to the DeepSpeech model file to use. Note that this must be compatible with the library passed to `libdeepspeech.so`.                                                                                                 | Yes       | None                                                      |
| `scorer-path`  | String                                   | The path to the DeepSpeech external scorer file to use.                                                                                                                                                                        | No        | The default scorer built in to DeepSpeech.                |
| `beam-width`   | Integer                                  | A field internal to Mozilla DeepSpeech that controls the speed-vs-accuracy amount. This should usually only be increased if the assistant is having trouble accurately matching spoken commands to the list of valid commands. | No        | 1                                                         |
| `commands`     | {`message` : String, `command` : String} | A list of `Command`s, each containing a `message` keyphrase that the assistant listens for and a `command` that the assistant runs as a sub-process if it hears the keyphrase.                                                 | Yes       | None                                                      |

An example is included in [`/res/config.toml`](/res/config.toml).

AssistantRS follows the FreeDesktop `XDG` config spec; it will read configuration options from the following paths in order, if they exist:

1. All paths passed via the `--configs` command line flag.
2. If the environment variable `XDG_CONFIG_HOME` is defined, then `$XDG_CONFIG_HOME/assistant-rs/assistant.toml`; otherwise `$HOME/.config/assistant-rs/assistant.toml` is read.
3. If the environment variable `XDG_CONFIG_DIRS` is defined, then it is treated as a list of directories separated by `:`; for each directory `$DIR` in this list, the config file `$DIR/assistant-rs/assistant.toml` is read; otherwise `/etc/xdg/assistant-rs/assistant.toml` is read. 
