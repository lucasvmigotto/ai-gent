// What the devcontainer plugin's hooks module keeps in $.state.

declare module 'claude-code' {
  interface PluginState {
    devcontainer: {
      // What was already shown this session, by name, so a reload repeats nothing.
      toasted: Record<string, boolean>
    }
  }
}
