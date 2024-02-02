# Add custom maps

The content server hosts a small upload app to allow uploading `pk3` or `zip` files containing maps. The content server in the [example.yaml](example.yaml) shares a volume with the game server, effectively "side-loading" the map content, however, in the future the game server will introspect into the maps and make sure that it can fulfill the users map configuration before starting.
