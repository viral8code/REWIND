#!/usr/bin/env python3
"""Independent real AT-SPI client for the native adapter acceptance test."""
import sys
import time
from gi.repository import Gio, GLib
session=Gio.bus_get_sync(Gio.BusType.SESSION,None)
reply=session.call_sync('org.a11y.Bus','/org/a11y/bus','org.a11y.Bus','GetAddress',None,None,0,5000,None)
connection=Gio.DBusConnection.new_for_address_sync(reply.unpack()[0],Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT|Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION,None,None)
bus=sys.argv[1]
# Use the system's actual AT-SPI client library as well as direct protocol checks.
import ctypes as C
atspi=C.CDLL('libatspi.so.0')
for name,result,args in [('atspi_get_desktop',C.c_void_p,[C.c_int]),('atspi_accessible_get_child_count',C.c_int,[C.c_void_p,C.c_void_p]),('atspi_accessible_get_child_at_index',C.c_void_p,[C.c_void_p,C.c_int,C.c_void_p]),('atspi_accessible_get_role',C.c_int,[C.c_void_p,C.c_void_p])]:
    function=getattr(atspi,name);function.restype=result;function.argtypes=args
assert atspi.atspi_init()==0
desktop=atspi.atspi_get_desktop(0)
assert desktop
apps=[atspi.atspi_accessible_get_child_at_index(desktop,i,None) for i in range(atspi.atspi_accessible_get_child_count(desktop,None))]
assert any(app and atspi.atspi_accessible_get_role(app,None)==75 for app in apps)

def call(path,interface,method,args=None,destination=bus):
    return connection.call_sync(destination,path,interface,method,args,None,0,5000,None).unpack()
def property(path,name):
    return call(path,'org.freedesktop.DBus.Properties','Get',GLib.Variant('(ss)',('org.a11y.atspi.Accessible',name)))[0]
registry='/org/a11y/atspi/accessible/root'
assert (bus,'/org/rewind/accessible/root') in call(registry,'org.a11y.atspi.Accessible','GetChildren',destination='org.a11y.atspi.Registry')[0]
window='/org/rewind/accessible/window'
children=call(window,'org.a11y.atspi.Accessible','GetChildren')[0]
assert len(children)==2
entry=children[1][1]
assert property(entry,"Name")=="Email address"
assert call(entry,"org.a11y.atspi.Accessible","GetRole")== (79,)
assert "org.a11y.atspi.Text" in call(entry,"org.a11y.atspi.Accessible","GetInterfaces")[0]
assert call(entry,"org.a11y.atspi.Text","GetText",GLib.Variant("(ii)",(1,2))) == ("🙂",)
assert call(entry,"org.a11y.atspi.Text","GetText",GLib.Variant("(ii)",(0,-1))) == ("界🙂abc",)
assert call(entry,"org.a11y.atspi.Text","GetNSelections")== (1,)
assert call(entry,"org.a11y.atspi.Text","GetSelection",GLib.Variant("(i)",(0,))) == (1,2)
path=children[0][1]
assert property(path,'Name')=='保存'
assert call(path,'org.a11y.atspi.Accessible','GetRole')==(43,)
notifications=[]
subscription=connection.signal_subscribe(bus,'org.a11y.atspi.Event.Object',None,path,None,0,lambda *args:notifications.append((args[4],args[5].unpack())),None)
assert call(path,'org.a11y.atspi.Action','DoAction',GLib.Variant('(i)',(0,)))==(True,)
deadline=time.monotonic()+5
while property(path,'Name')!='Disabled':
    assert time.monotonic()<deadline
    time.sleep(.005)
state=call(path,'org.a11y.atspi.Accessible','GetState')[0]
assert not state[0] & (1<<8)
assert call(path,'org.a11y.atspi.Action','DoAction',GLib.Variant('(i)',(0,)))==(False,)
context=GLib.MainContext.default()
while context.pending():context.iteration(False)
assert any(kind=='PropertyChange' and data[0]=='accessible-name' for kind,data in notifications),notifications
assert any(kind=='StateChanged' and data[0]=='enabled' and data[1]==0 for kind,data in notifications),notifications
connection.signal_unsubscribe(subscription)
print('Verified real AT-SPI registry, Unicode name, native role/state, change notifications and dispatched action')
