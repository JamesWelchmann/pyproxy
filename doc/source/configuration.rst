
Server Configuration
-----------------------

All PyProxy Server configuration is done with environment variables.

Below is an exhaustive list of configuration variables along with their defaults.

PYPROXY_BIND_ADDR 
~~~~~~~~~~~~~~~~~~~

``DEFAULT: 0.0.0.0:9000``

The address on which the mainstream will bind.

Example:

``PYPROXY_BIND_ADDR=0.0.0.0:6000``

PYPROXY_OUTPUT_ADDR 
~~~~~~~~~~~~~~~~~~~~~~

``DEFAULT: 0.0.0.0:9000``

The address on which the outputstream will bind.

Example:

``PYPROXY_OUTPUT_ADDR=0.0.0.0:6000``

PYPROXY_NUM_WORKERS 
~~~~~~~~~~~~~~~~~~~~~~

``DEFAULT: 3``

The number of worker processes PyProxy server will run.

Example:

``PYPROXY_NUM_WORKERS=5``
