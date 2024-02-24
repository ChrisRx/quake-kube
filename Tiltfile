docker_build('quake-kube', '.', 
    dockerfile='Dockerfile')
k8s_yaml(
  helm('./chart',
    name='quake-kube',
    set=[
      'image.repository=quake-kube',
    ]
  )
)

# Using tilt port_forward doesn't work with graceful pod termination, the
# port_forward is closed as soon as the deployment changes.
k8s_resource('quake-kube-chart', port_forwards='30001:8080')
