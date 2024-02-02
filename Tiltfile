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
k8s_resource('quake-kube-chart', port_forwards='30001:8080')
