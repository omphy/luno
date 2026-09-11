local co = coroutine.create(function(var)
    local co2 = coroutine.create(function(var)
        print(var)
        coroutine.yield()
        print(var .. " 2.0")
        coroutine.yield(5)
        local a = "a"
        local b = "a"
        print(a == b)
    end)

    coroutine.resume(co2, "hello")
    print(coroutine.resume(co2))
    coroutine.resume(co2)

    print(var)
    coroutine.yield()
    print(var .. " 2.0")
end)

coroutine.resume(co, "hi")
coroutine.resume(co)