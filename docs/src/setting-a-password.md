# Setting a password

To require a password to join a server:

```yaml
spec:
  game:
    password: "letmein"
```

The web app's **Password** field sets it for servers it creates. The browser
client asks for it when a server needs one.

It's stored as plain text in the QuakeServer, so anyone who can read QuakeServers
can read it (see [Security](security.md#passwords)).

## Remote administration

The controller generates an rcon password for each server and keeps it in the
server's Secret, which has the same name as the server:

```shell
kubectl get secret <server> -o jsonpath='{.data.rconPassword}' | base64 -d
```

A native client can then administer the server from its console with
`\rcon <password> <command>`.
