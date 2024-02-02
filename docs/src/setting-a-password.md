# Setting a password

A password should be set for the server to allow remote administration and is found in the server configuration settings:

```yaml
server:
  password: "changeme"
```

This will allow clients to use `\rcon changeme <cmd>` to remotely administrate the server. To create a password that must be provided by clients to connect:

```yaml
game:
  password: "letmein"
```

This will add an additional dialog to the in-browser client to accept the password. It will only appear if the server indicates it needs a password.
