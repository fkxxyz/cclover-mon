#ifndef CCLOVER_LINUX_DBUSMENU_CONTRACT_H
#define CCLOVER_LINUX_DBUSMENU_CONTRACT_H

#define CCLOVER_DBUSMENU_METHODS(X) \
    X(GET_LAYOUT, "GetLayout", \
      "  <method name='GetLayout'>" \
      "   <arg type='i' direction='in'/><arg type='i' direction='in'/><arg type='as' direction='in'/>" \
      "   <arg type='u' direction='out'/><arg type='(ia{sv}av)' direction='out'/>" \
      "  </method>") \
    X(GET_GROUP_PROPERTIES, "GetGroupProperties", \
      "  <method name='GetGroupProperties'>" \
      "   <arg type='ai' direction='in'/><arg type='as' direction='in'/><arg type='a(ia{sv})' direction='out'/>" \
      "  </method>") \
    X(GET_PROPERTY, "GetProperty", \
      "  <method name='GetProperty'>" \
      "   <arg type='i' direction='in'/><arg type='s' direction='in'/><arg type='v' direction='out'/>" \
      "  </method>") \
    X(EVENT, "Event", \
      "  <method name='Event'>" \
      "   <arg type='i' direction='in'/><arg type='s' direction='in'/><arg type='v' direction='in'/><arg type='u' direction='in'/>" \
      "  </method>") \
    X(EVENT_GROUP, "EventGroup", \
      "  <method name='EventGroup'>" \
      "   <arg type='a(isvu)' direction='in'/><arg type='ai' direction='out'/>" \
      "  </method>") \
    X(ABOUT_TO_SHOW, "AboutToShow", \
      "  <method name='AboutToShow'><arg type='i' direction='in'/><arg type='b' direction='out'/></method>") \
    X(ABOUT_TO_SHOW_GROUP, "AboutToShowGroup", \
      "  <method name='AboutToShowGroup'>" \
      "   <arg type='ai' direction='in'/><arg type='ai' direction='out'/><arg type='ai' direction='out'/>" \
      "  </method>")

#endif
