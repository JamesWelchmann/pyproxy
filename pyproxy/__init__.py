"""
pyproxy is the client library for the PyProxy Server
"""

__author__ = ('James Welchman',)

from .remote_proc import PyProxySession, RemoteProcess
from .future import Future

__all__ = [
    'RemoteProcess',
    'PyProxySession',
    'Future',
]
