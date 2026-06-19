var x = 0;

if (true) {
    x = 1;
} else {
    x = 2;
}

var i = 0;
while (i < 5) {
    i = i + 1;

    if (i === 3) {
        continue;
    }

    if (i === 4) {
        break;
    }
}

x + i;