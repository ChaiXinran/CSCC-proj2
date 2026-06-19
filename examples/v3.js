function outer(x) {
    function inner(y) {
        return x + y;
    }

    return inner(2);
}

var object = {
    value: outer(1),
    get: function () {
        return this.value;
    }
};

var array = [object.get(), 4];
array[0] + array.length;
