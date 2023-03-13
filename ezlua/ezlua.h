
#ifndef __EZLUA_H__
#define __EZLUA_H__

#define lua_lock(L) ezlua_lock(L)
#define lua_unlock(L) ezlua_unlock(L)
#define luai_userstateopen(L) ezlua_userstateopen(L)
#define luai_userstateclose(L) ezlua_userstateclose(L)

extern void ezlua_lock(lua_State*);
extern void ezlua_unlock(lua_State*);
extern void ezlua_userstateopen(lua_State*);
extern void ezlua_userstateclose(lua_State*);

#endif /* __EZLUA_H__ */
