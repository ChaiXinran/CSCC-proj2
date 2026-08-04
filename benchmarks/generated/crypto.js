
const isInBrowser = false;
const isD8 = false;
const isSpiderMonkey = false;
const jetStreamHostPrint = typeof globalThis.print === "function"
    ? globalThis.print
    : (...args) => globalThis.console.log(...args);
globalThis.print = jetStreamHostPrint;
var __jetstreamPhase = (phase) =>
    jetStreamHostPrint("JETSTREAM_PHASE:" + Date.now() + ":" + phase);
var console = {
    log: (...args) => jetStreamHostPrint(...args),
    warn: (...args) => jetStreamHostPrint(...args),
    error: (...args) => jetStreamHostPrint(...args),
    assert(condition, ...args) {
        if (!condition)
            throw new Error(args.join(" ") || "Assertion failed");
    },
};
var runString = () => {
    globalThis.loadString = (source) =>
        new Function("top", source)(globalThis.top);
    return globalThis;
};
var load = (name) => globalThis.loadString(readFile(name));
var performance = globalThis.performance = {
    now: Date.now.bind(Date),
    mark(name) { return { name }; },
    measure() {},
};
var document = globalThis.document = {
    getElementById() { return { innerHTML: "" }; }
};
var testList = "crypto";
var testIterationCount = 1;
var RAMification = false;
var JetStreamParams = {
    prefetchResources: false,
    forceGC: false,
    dumpJSONResults: false,
    testIterationCount: 1,
    testWorstCaseCount: 0,
    testIterationCountMap: {},
    testWorstCaseCountMap: {},
    testList: "crypto",
};
var __jetstreamResources = {"./Octane/crypto.js":"/*\n * Copyright (c) 2003-2005  Tom Wu\n * All Rights Reserved.\n *\n * Permission is hereby granted, free of charge, to any person obtaining\n * a copy of this software and associated documentation files (the\n * \"Software\"), to deal in the Software without restriction, including\n * without limitation the rights to use, copy, modify, merge, publish,\n * distribute, sublicense, and/or sell copies of the Software, and to\n * permit persons to whom the Software is furnished to do so, subject to\n * the following conditions:\n *\n * The above copyright notice and this permission notice shall be\n * included in all copies or substantial portions of the Software.\n *\n * THE SOFTWARE IS PROVIDED \"AS-IS\" AND WITHOUT WARRANTY OF ANY KIND,\n * EXPRESS, IMPLIED OR OTHERWISE, INCLUDING WITHOUT LIMITATION, ANY\n * WARRANTY OF MERCHANTABILITY OR FITNESS FOR A PARTICULAR PURPOSE.\n *\n * IN NO EVENT SHALL TOM WU BE LIABLE FOR ANY SPECIAL, INCIDENTAL,\n * INDIRECT OR CONSEQUENTIAL DAMAGES OF ANY KIND, OR ANY DAMAGES WHATSOEVER\n * RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER OR NOT ADVISED OF\n * THE POSSIBILITY OF DAMAGE, AND ON ANY THEORY OF LIABILITY, ARISING OUT\n * OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.\n *\n * In addition, the following condition applies:\n *\n * All redistributions must retain an intact copy of this copyright notice\n * and disclaimer.\n */\n\n\n// The code has been adapted for use as a benchmark by Google.\n\n// Basic JavaScript BN library - subset useful for RSA encryption.\n\n// Bits per digit\nvar dbits;\nvar BI_DB;\nvar BI_DM;\nvar BI_DV;\n\nvar BI_FP;\nvar BI_FV;\nvar BI_F1;\nvar BI_F2;\n\n// JavaScript engine analysis\nvar canary = 0xdeadbeefcafe;\nvar j_lm = ((canary&0xffffff)==0xefcafe);\n\n// (public) Constructor\nfunction BigInteger(a,b,c) {\n  this.array = new Array();\n  if(a != null)\n    if(\"number\" == typeof a) this.fromNumber(a,b,c);\n    else if(b == null && \"string\" != typeof a) this.fromString(a,256);\n    else this.fromString(a,b);\n}\n\n// return new, unset BigInteger\nfunction nbi() { return new BigInteger(null); }\n\n// am: Compute w_j += (x*this_i), propagate carries,\n// c is initial carry, returns final carry.\n// c < 3*dvalue, x < 2*dvalue, this_i < dvalue\n// We need to select the fastest one that works in this environment.\n\n// am1: use a single mult and divide to get the high bits,\n// max digit bits should be 26 because\n// max internal value = 2*dvalue^2-2*dvalue (< 2^53)\nfunction am1(i,x,w,j,c,n) {\n  var this_array = this.array;\n  var w_array    = w.array;\n  while(--n >= 0) {\n    var v = x*this_array[i++]+w_array[j]+c;\n    c = Math.floor(v/0x4000000);\n    w_array[j++] = v&0x3ffffff;\n  }\n  return c;\n}\n\n// am2 avoids a big mult-and-extract completely.\n// Max digit bits should be <= 30 because we do bitwise ops\n// on values up to 2*hdvalue^2-hdvalue-1 (< 2^31)\nfunction am2(i,x,w,j,c,n) {\n  var this_array = this.array;\n  var w_array    = w.array;\n  var xl = x&0x7fff, xh = x>>15;\n  while(--n >= 0) {\n    var l = this_array[i]&0x7fff;\n    var h = this_array[i++]>>15;\n    var m = xh*l+h*xl;\n    l = xl*l+((m&0x7fff)<<15)+w_array[j]+(c&0x3fffffff);\n    c = (l>>>30)+(m>>>15)+xh*h+(c>>>30);\n    w_array[j++] = l&0x3fffffff;\n  }\n  return c;\n}\n\n// Alternately, set max digit bits to 28 since some\n// browsers slow down when dealing with 32-bit numbers.\nfunction am3(i,x,w,j,c,n) {\n  var this_array = this.array;\n  var w_array    = w.array;\n\n  var xl = x&0x3fff, xh = x>>14;\n  while(--n >= 0) {\n    var l = this_array[i]&0x3fff;\n    var h = this_array[i++]>>14;\n    var m = xh*l+h*xl;\n    l = xl*l+((m&0x3fff)<<14)+w_array[j]+c;\n    c = (l>>28)+(m>>14)+xh*h;\n    w_array[j++] = l&0xfffffff;\n  }\n  return c;\n}\n\n// This is tailored to VMs with 2-bit tagging. It makes sure\n// that all the computations stay within the 29 bits available.\nfunction am4(i,x,w,j,c,n) {\n  var this_array = this.array;\n  var w_array    = w.array;\n\n  var xl = x&0x1fff, xh = x>>13;\n  while(--n >= 0) {\n    var l = this_array[i]&0x1fff;\n    var h = this_array[i++]>>13;\n    var m = xh*l+h*xl;\n    l = xl*l+((m&0x1fff)<<13)+w_array[j]+c;\n    c = (l>>26)+(m>>13)+xh*h;\n    w_array[j++] = l&0x3ffffff;\n  }\n  return c;\n}\n\n// am3/28 is best for SM, Rhino, but am4/26 is best for v8.\n// Kestrel (Opera 9.5) gets its best result with am4/26.\n// IE7 does 9% better with am3/28 than with am4/26.\n// Firefox (SM) gets 10% faster with am3/28 than with am4/26.\n\nsetupEngine = function(fn, bits) {\n  BigInteger.prototype.am = fn;\n  dbits = bits;\n\n  BI_DB = dbits;\n  BI_DM = ((1<<dbits)-1);\n  BI_DV = (1<<dbits);\n\n  BI_FP = 52;\n  BI_FV = Math.pow(2,BI_FP);\n  BI_F1 = BI_FP-dbits;\n  BI_F2 = 2*dbits-BI_FP;\n}\n\n\n// Digit conversions\nvar BI_RM = \"0123456789abcdefghijklmnopqrstuvwxyz\";\nvar BI_RC = new Array();\nvar rr,vv;\nrr = \"0\".charCodeAt(0);\nfor(vv = 0; vv <= 9; ++vv) BI_RC[rr++] = vv;\nrr = \"a\".charCodeAt(0);\nfor(vv = 10; vv < 36; ++vv) BI_RC[rr++] = vv;\nrr = \"A\".charCodeAt(0);\nfor(vv = 10; vv < 36; ++vv) BI_RC[rr++] = vv;\n\nfunction int2char(n) { return BI_RM.charAt(n); }\nfunction intAt(s,i) {\n  var c = BI_RC[s.charCodeAt(i)];\n  return (c==null)?-1:c;\n}\n\n// (protected) copy this to r\nfunction bnpCopyTo(r) {\n  var this_array = this.array;\n  var r_array    = r.array;\n\n  for(var i = this.t-1; i >= 0; --i) r_array[i] = this_array[i];\n  r.t = this.t;\n  r.s = this.s;\n}\n\n// (protected) set from integer value x, -DV <= x < DV\nfunction bnpFromInt(x) {\n  var this_array = this.array;\n  this.t = 1;\n  this.s = (x<0)?-1:0;\n  if(x > 0) this_array[0] = x;\n  else if(x < -1) this_array[0] = x+DV;\n  else this.t = 0;\n}\n\n// return bigint initialized to value\nfunction nbv(i) { var r = nbi(); r.fromInt(i); return r; }\n\n// (protected) set from string and radix\nfunction bnpFromString(s,b) {\n  var this_array = this.array;\n  var k;\n  if(b == 16) k = 4;\n  else if(b == 8) k = 3;\n  else if(b == 256) k = 8; // byte array\n  else if(b == 2) k = 1;\n  else if(b == 32) k = 5;\n  else if(b == 4) k = 2;\n  else { this.fromRadix(s,b); return; }\n  this.t = 0;\n  this.s = 0;\n  var i = s.length, mi = false, sh = 0;\n  while(--i >= 0) {\n    var x = (k==8)?s[i]&0xff:intAt(s,i);\n    if(x < 0) {\n      if(s.charAt(i) == \"-\") mi = true;\n      continue;\n    }\n    mi = false;\n    if(sh == 0)\n      this_array[this.t++] = x;\n    else if(sh+k > BI_DB) {\n      this_array[this.t-1] |= (x&((1<<(BI_DB-sh))-1))<<sh;\n      this_array[this.t++] = (x>>(BI_DB-sh));\n    }\n    else\n      this_array[this.t-1] |= x<<sh;\n    sh += k;\n    if(sh >= BI_DB) sh -= BI_DB;\n  }\n  if(k == 8 && (s[0]&0x80) != 0) {\n    this.s = -1;\n    if(sh > 0) this_array[this.t-1] |= ((1<<(BI_DB-sh))-1)<<sh;\n  }\n  this.clamp();\n  if(mi) BigInteger.ZERO.subTo(this,this);\n}\n\n// (protected) clamp off excess high words\nfunction bnpClamp() {\n  var this_array = this.array;\n  var c = this.s&BI_DM;\n  while(this.t > 0 && this_array[this.t-1] == c) --this.t;\n}\n\n// (public) return string representation in given radix\nfunction bnToString(b) {\n  var this_array = this.array;\n  if(this.s < 0) return \"-\"+this.negate().toString(b);\n  var k;\n  if(b == 16) k = 4;\n  else if(b == 8) k = 3;\n  else if(b == 2) k = 1;\n  else if(b == 32) k = 5;\n  else if(b == 4) k = 2;\n  else return this.toRadix(b);\n  var km = (1<<k)-1, d, m = false, r = \"\", i = this.t;\n  var p = BI_DB-(i*BI_DB)%k;\n  if(i-- > 0) {\n    if(p < BI_DB && (d = this_array[i]>>p) > 0) { m = true; r = int2char(d); }\n    while(i >= 0) {\n      if(p < k) {\n        d = (this_array[i]&((1<<p)-1))<<(k-p);\n        d |= this_array[--i]>>(p+=BI_DB-k);\n      }\n      else {\n        d = (this_array[i]>>(p-=k))&km;\n        if(p <= 0) { p += BI_DB; --i; }\n      }\n      if(d > 0) m = true;\n      if(m) r += int2char(d);\n    }\n  }\n  return m?r:\"0\";\n}\n\n// (public) -this\nfunction bnNegate() { var r = nbi(); BigInteger.ZERO.subTo(this,r); return r; }\n\n// (public) |this|\nfunction bnAbs() { return (this.s<0)?this.negate():this; }\n\n// (public) return + if this > a, - if this < a, 0 if equal\nfunction bnCompareTo(a) {\n  var this_array = this.array;\n  var a_array = a.array;\n\n  var r = this.s-a.s;\n  if(r != 0) return r;\n  var i = this.t;\n  r = i-a.t;\n  if(r != 0) return r;\n  while(--i >= 0) if((r=this_array[i]-a_array[i]) != 0) return r;\n  return 0;\n}\n\n// returns bit length of the integer x\nfunction nbits(x) {\n  var r = 1, t;\n  if((t=x>>>16) != 0) { x = t; r += 16; }\n  if((t=x>>8) != 0) { x = t; r += 8; }\n  if((t=x>>4) != 0) { x = t; r += 4; }\n  if((t=x>>2) != 0) { x = t; r += 2; }\n  if((t=x>>1) != 0) { x = t; r += 1; }\n  return r;\n}\n\n// (public) return the number of bits in \"this\"\nfunction bnBitLength() {\n  var this_array = this.array;\n  if(this.t <= 0) return 0;\n  return BI_DB*(this.t-1)+nbits(this_array[this.t-1]^(this.s&BI_DM));\n}\n\n// (protected) r = this << n*DB\nfunction bnpDLShiftTo(n,r) {\n  var this_array = this.array;\n  var r_array = r.array;\n  var i;\n  for(i = this.t-1; i >= 0; --i) r_array[i+n] = this_array[i];\n  for(i = n-1; i >= 0; --i) r_array[i] = 0;\n  r.t = this.t+n;\n  r.s = this.s;\n}\n\n// (protected) r = this >> n*DB\nfunction bnpDRShiftTo(n,r) {\n  var this_array = this.array;\n  var r_array = r.array;\n  for(var i = n; i < this.t; ++i) r_array[i-n] = this_array[i];\n  r.t = Math.max(this.t-n,0);\n  r.s = this.s;\n}\n\n// (protected) r = this << n\nfunction bnpLShiftTo(n,r) {\n  var this_array = this.array;\n  var r_array = r.array;\n  var bs = n%BI_DB;\n  var cbs = BI_DB-bs;\n  var bm = (1<<cbs)-1;\n  var ds = Math.floor(n/BI_DB), c = (this.s<<bs)&BI_DM, i;\n  for(i = this.t-1; i >= 0; --i) {\n    r_array[i+ds+1] = (this_array[i]>>cbs)|c;\n    c = (this_array[i]&bm)<<bs;\n  }\n  for(i = ds-1; i >= 0; --i) r_array[i] = 0;\n  r_array[ds] = c;\n  r.t = this.t+ds+1;\n  r.s = this.s;\n  r.clamp();\n}\n\n// (protected) r = this >> n\nfunction bnpRShiftTo(n,r) {\n  var this_array = this.array;\n  var r_array = r.array;\n  r.s = this.s;\n  var ds = Math.floor(n/BI_DB);\n  if(ds >= this.t) { r.t = 0; return; }\n  var bs = n%BI_DB;\n  var cbs = BI_DB-bs;\n  var bm = (1<<bs)-1;\n  r_array[0] = this_array[ds]>>bs;\n  for(var i = ds+1; i < this.t; ++i) {\n    r_array[i-ds-1] |= (this_array[i]&bm)<<cbs;\n    r_array[i-ds] = this_array[i]>>bs;\n  }\n  if(bs > 0) r_array[this.t-ds-1] |= (this.s&bm)<<cbs;\n  r.t = this.t-ds;\n  r.clamp();\n}\n\n// (protected) r = this - a\nfunction bnpSubTo(a,r) {\n  var this_array = this.array;\n  var r_array = r.array;\n  var a_array = a.array;\n  var i = 0, c = 0, m = Math.min(a.t,this.t);\n  while(i < m) {\n    c += this_array[i]-a_array[i];\n    r_array[i++] = c&BI_DM;\n    c >>= BI_DB;\n  }\n  if(a.t < this.t) {\n    c -= a.s;\n    while(i < this.t) {\n      c += this_array[i];\n      r_array[i++] = c&BI_DM;\n      c >>= BI_DB;\n    }\n    c += this.s;\n  }\n  else {\n    c += this.s;\n    while(i < a.t) {\n      c -= a_array[i];\n      r_array[i++] = c&BI_DM;\n      c >>= BI_DB;\n    }\n    c -= a.s;\n  }\n  r.s = (c<0)?-1:0;\n  if(c < -1) r_array[i++] = BI_DV+c;\n  else if(c > 0) r_array[i++] = c;\n  r.t = i;\n  r.clamp();\n}\n\n// (protected) r = this * a, r != this,a (HAC 14.12)\n// \"this\" should be the larger one if appropriate.\nfunction bnpMultiplyTo(a,r) {\n  var this_array = this.array;\n  var r_array = r.array;\n  var x = this.abs(), y = a.abs();\n  var y_array = y.array;\n\n  var i = x.t;\n  r.t = i+y.t;\n  while(--i >= 0) r_array[i] = 0;\n  for(i = 0; i < y.t; ++i) r_array[i+x.t] = x.am(0,y_array[i],r,i,0,x.t);\n  r.s = 0;\n  r.clamp();\n  if(this.s != a.s) BigInteger.ZERO.subTo(r,r);\n}\n\n// (protected) r = this^2, r != this (HAC 14.16)\nfunction bnpSquareTo(r) {\n  var x = this.abs();\n  var x_array = x.array;\n  var r_array = r.array;\n\n  var i = r.t = 2*x.t;\n  while(--i >= 0) r_array[i] = 0;\n  for(i = 0; i < x.t-1; ++i) {\n    var c = x.am(i,x_array[i],r,2*i,0,1);\n    if((r_array[i+x.t]+=x.am(i+1,2*x_array[i],r,2*i+1,c,x.t-i-1)) >= BI_DV) {\n      r_array[i+x.t] -= BI_DV;\n      r_array[i+x.t+1] = 1;\n    }\n  }\n  if(r.t > 0) r_array[r.t-1] += x.am(i,x_array[i],r,2*i,0,1);\n  r.s = 0;\n  r.clamp();\n}\n\n// (protected) divide this by m, quotient and remainder to q, r (HAC 14.20)\n// r != q, this != m.  q or r may be null.\nfunction bnpDivRemTo(m,q,r) {\n  var pm = m.abs();\n  if(pm.t <= 0) return;\n  var pt = this.abs();\n  if(pt.t < pm.t) {\n    if(q != null) q.fromInt(0);\n    if(r != null) this.copyTo(r);\n    return;\n  }\n  if(r == null) r = nbi();\n  var y = nbi(), ts = this.s, ms = m.s;\n  var pm_array = pm.array;\n  var nsh = BI_DB-nbits(pm_array[pm.t-1]);\t// normalize modulus\n  if(nsh > 0) { pm.lShiftTo(nsh,y); pt.lShiftTo(nsh,r); }\n  else { pm.copyTo(y); pt.copyTo(r); }\n  var ys = y.t;\n\n  var y_array = y.array;\n  var y0 = y_array[ys-1];\n  if(y0 == 0) return;\n  var yt = y0*(1<<BI_F1)+((ys>1)?y_array[ys-2]>>BI_F2:0);\n  var d1 = BI_FV/yt, d2 = (1<<BI_F1)/yt, e = 1<<BI_F2;\n  var i = r.t, j = i-ys, t = (q==null)?nbi():q;\n  y.dlShiftTo(j,t);\n\n  var r_array = r.array;\n  if(r.compareTo(t) >= 0) {\n    r_array[r.t++] = 1;\n    r.subTo(t,r);\n  }\n  BigInteger.ONE.dlShiftTo(ys,t);\n  t.subTo(y,y);\t// \"negative\" y so we can replace sub with am later\n  while(y.t < ys) y_array[y.t++] = 0;\n  while(--j >= 0) {\n    // Estimate quotient digit\n    var qd = (r_array[--i]==y0)?BI_DM:Math.floor(r_array[i]*d1+(r_array[i-1]+e)*d2);\n    if((r_array[i]+=y.am(0,qd,r,j,0,ys)) < qd) {\t// Try it out\n      y.dlShiftTo(j,t);\n      r.subTo(t,r);\n      while(r_array[i] < --qd) r.subTo(t,r);\n    }\n  }\n  if(q != null) {\n    r.drShiftTo(ys,q);\n    if(ts != ms) BigInteger.ZERO.subTo(q,q);\n  }\n  r.t = ys;\n  r.clamp();\n  if(nsh > 0) r.rShiftTo(nsh,r);\t// Denormalize remainder\n  if(ts < 0) BigInteger.ZERO.subTo(r,r);\n}\n\n// (public) this mod a\nfunction bnMod(a) {\n  var r = nbi();\n  this.abs().divRemTo(a,null,r);\n  if(this.s < 0 && r.compareTo(BigInteger.ZERO) > 0) a.subTo(r,r);\n  return r;\n}\n\n// Modular reduction using \"classic\" algorithm\nfunction Classic(m) { this.m = m; }\nfunction cConvert(x) {\n  if(x.s < 0 || x.compareTo(this.m) >= 0) return x.mod(this.m);\n  else return x;\n}\nfunction cRevert(x) { return x; }\nfunction cReduce(x) { x.divRemTo(this.m,null,x); }\nfunction cMulTo(x,y,r) { x.multiplyTo(y,r); this.reduce(r); }\nfunction cSqrTo(x,r) { x.squareTo(r); this.reduce(r); }\n\nClassic.prototype.convert = cConvert;\nClassic.prototype.revert = cRevert;\nClassic.prototype.reduce = cReduce;\nClassic.prototype.mulTo = cMulTo;\nClassic.prototype.sqrTo = cSqrTo;\n\n// (protected) return \"-1/this % 2^DB\"; useful for Mont. reduction\n// justification:\n//         xy == 1 (mod m)\n//         xy =  1+km\n//   xy(2-xy) = (1+km)(1-km)\n// x[y(2-xy)] = 1-k^2m^2\n// x[y(2-xy)] == 1 (mod m^2)\n// if y is 1/x mod m, then y(2-xy) is 1/x mod m^2\n// should reduce x and y(2-xy) by m^2 at each step to keep size bounded.\n// JS multiply \"overflows\" differently from C/C++, so care is needed here.\nfunction bnpInvDigit() {\n  var this_array = this.array;\n  if(this.t < 1) return 0;\n  var x = this_array[0];\n  if((x&1) == 0) return 0;\n  var y = x&3;\t\t// y == 1/x mod 2^2\n  y = (y*(2-(x&0xf)*y))&0xf;\t// y == 1/x mod 2^4\n  y = (y*(2-(x&0xff)*y))&0xff;\t// y == 1/x mod 2^8\n  y = (y*(2-(((x&0xffff)*y)&0xffff)))&0xffff;\t// y == 1/x mod 2^16\n  // last step - calculate inverse mod DV directly;\n  // assumes 16 < DB <= 32 and assumes ability to handle 48-bit ints\n  y = (y*(2-x*y%BI_DV))%BI_DV;\t\t// y == 1/x mod 2^dbits\n  // we really want the negative inverse, and -DV < y < DV\n  return (y>0)?BI_DV-y:-y;\n}\n\n// Montgomery reduction\nfunction Montgomery(m) {\n  this.m = m;\n  this.mp = m.invDigit();\n  this.mpl = this.mp&0x7fff;\n  this.mph = this.mp>>15;\n  this.um = (1<<(BI_DB-15))-1;\n  this.mt2 = 2*m.t;\n}\n\n// xR mod m\nfunction montConvert(x) {\n  var r = nbi();\n  x.abs().dlShiftTo(this.m.t,r);\n  r.divRemTo(this.m,null,r);\n  if(x.s < 0 && r.compareTo(BigInteger.ZERO) > 0) this.m.subTo(r,r);\n  return r;\n}\n\n// x/R mod m\nfunction montRevert(x) {\n  var r = nbi();\n  x.copyTo(r);\n  this.reduce(r);\n  return r;\n}\n\n// x = x/R mod m (HAC 14.32)\nfunction montReduce(x) {\n  var x_array = x.array;\n  while(x.t <= this.mt2)\t// pad x so am has enough room later\n    x_array[x.t++] = 0;\n  for(var i = 0; i < this.m.t; ++i) {\n    // faster way of calculating u0 = x[i]*mp mod DV\n    var j = x_array[i]&0x7fff;\n    var u0 = (j*this.mpl+(((j*this.mph+(x_array[i]>>15)*this.mpl)&this.um)<<15))&BI_DM;\n    // use am to combine the multiply-shift-add into one call\n    j = i+this.m.t;\n    x_array[j] += this.m.am(0,u0,x,i,0,this.m.t);\n    // propagate carry\n    while(x_array[j] >= BI_DV) { x_array[j] -= BI_DV; x_array[++j]++; }\n  }\n  x.clamp();\n  x.drShiftTo(this.m.t,x);\n  if(x.compareTo(this.m) >= 0) x.subTo(this.m,x);\n}\n\n// r = \"x^2/R mod m\"; x != r\nfunction montSqrTo(x,r) { x.squareTo(r); this.reduce(r); }\n\n// r = \"xy/R mod m\"; x,y != r\nfunction montMulTo(x,y,r) { x.multiplyTo(y,r); this.reduce(r); }\n\nMontgomery.prototype.convert = montConvert;\nMontgomery.prototype.revert = montRevert;\nMontgomery.prototype.reduce = montReduce;\nMontgomery.prototype.mulTo = montMulTo;\nMontgomery.prototype.sqrTo = montSqrTo;\n\n// (protected) true iff this is even\nfunction bnpIsEven() {\n  var this_array = this.array;\n  return ((this.t>0)?(this_array[0]&1):this.s) == 0;\n}\n\n// (protected) this^e, e < 2^32, doing sqr and mul with \"r\" (HAC 14.79)\nfunction bnpExp(e,z) {\n  if(e > 0xffffffff || e < 1) return BigInteger.ONE;\n  var r = nbi(), r2 = nbi(), g = z.convert(this), i = nbits(e)-1;\n  g.copyTo(r);\n  while(--i >= 0) {\n    z.sqrTo(r,r2);\n    if((e&(1<<i)) > 0) z.mulTo(r2,g,r);\n    else { var t = r; r = r2; r2 = t; }\n  }\n  return z.revert(r);\n}\n\n// (public) this^e % m, 0 <= e < 2^32\nfunction bnModPowInt(e,m) {\n  var z;\n  if(e < 256 || m.isEven()) z = new Classic(m); else z = new Montgomery(m);\n  return this.exp(e,z);\n}\n\n// protected\nBigInteger.prototype.copyTo = bnpCopyTo;\nBigInteger.prototype.fromInt = bnpFromInt;\nBigInteger.prototype.fromString = bnpFromString;\nBigInteger.prototype.clamp = bnpClamp;\nBigInteger.prototype.dlShiftTo = bnpDLShiftTo;\nBigInteger.prototype.drShiftTo = bnpDRShiftTo;\nBigInteger.prototype.lShiftTo = bnpLShiftTo;\nBigInteger.prototype.rShiftTo = bnpRShiftTo;\nBigInteger.prototype.subTo = bnpSubTo;\nBigInteger.prototype.multiplyTo = bnpMultiplyTo;\nBigInteger.prototype.squareTo = bnpSquareTo;\nBigInteger.prototype.divRemTo = bnpDivRemTo;\nBigInteger.prototype.invDigit = bnpInvDigit;\nBigInteger.prototype.isEven = bnpIsEven;\nBigInteger.prototype.exp = bnpExp;\n\n// public\nBigInteger.prototype.toString = bnToString;\nBigInteger.prototype.negate = bnNegate;\nBigInteger.prototype.abs = bnAbs;\nBigInteger.prototype.compareTo = bnCompareTo;\nBigInteger.prototype.bitLength = bnBitLength;\nBigInteger.prototype.mod = bnMod;\nBigInteger.prototype.modPowInt = bnModPowInt;\n\n// \"constants\"\nBigInteger.ZERO = nbv(0);\nBigInteger.ONE = nbv(1);\n// Copyright (c) 2005  Tom Wu\n// All Rights Reserved.\n// See \"LICENSE\" for details.\n\n// Extended JavaScript BN functions, required for RSA private ops.\n\n// (public)\nfunction bnClone() { var r = nbi(); this.copyTo(r); return r; }\n\n// (public) return value as integer\nfunction bnIntValue() {\n  var this_array = this.array;\n  if(this.s < 0) {\n    if(this.t == 1) return this_array[0]-BI_DV;\n    else if(this.t == 0) return -1;\n  }\n  else if(this.t == 1) return this_array[0];\n  else if(this.t == 0) return 0;\n  // assumes 16 < DB < 32\n  return ((this_array[1]&((1<<(32-BI_DB))-1))<<BI_DB)|this_array[0];\n}\n\n// (public) return value as byte\nfunction bnByteValue() {\n  var this_array = this.array;\n  return (this.t==0)?this.s:(this_array[0]<<24)>>24;\n}\n\n// (public) return value as short (assumes DB>=16)\nfunction bnShortValue() {\n  var this_array = this.array;\n  return (this.t==0)?this.s:(this_array[0]<<16)>>16;\n}\n\n// (protected) return x s.t. r^x < DV\nfunction bnpChunkSize(r) { return Math.floor(Math.LN2*BI_DB/Math.log(r)); }\n\n// (public) 0 if this == 0, 1 if this > 0\nfunction bnSigNum() {\n  var this_array = this.array;\n  if(this.s < 0) return -1;\n  else if(this.t <= 0 || (this.t == 1 && this_array[0] <= 0)) return 0;\n  else return 1;\n}\n\n// (protected) convert to radix string\nfunction bnpToRadix(b) {\n  if(b == null) b = 10;\n  if(this.signum() == 0 || b < 2 || b > 36) return \"0\";\n  var cs = this.chunkSize(b);\n  var a = Math.pow(b,cs);\n  var d = nbv(a), y = nbi(), z = nbi(), r = \"\";\n  this.divRemTo(d,y,z);\n  while(y.signum() > 0) {\n    r = (a+z.intValue()).toString(b).substr(1) + r;\n    y.divRemTo(d,y,z);\n  }\n  return z.intValue().toString(b) + r;\n}\n\n// (protected) convert from radix string\nfunction bnpFromRadix(s,b) {\n  this.fromInt(0);\n  if(b == null) b = 10;\n  var cs = this.chunkSize(b);\n  var d = Math.pow(b,cs), mi = false, j = 0, w = 0;\n  for(var i = 0; i < s.length; ++i) {\n    var x = intAt(s,i);\n    if(x < 0) {\n      if(s.charAt(i) == \"-\" && this.signum() == 0) mi = true;\n      continue;\n    }\n    w = b*w+x;\n    if(++j >= cs) {\n      this.dMultiply(d);\n      this.dAddOffset(w,0);\n      j = 0;\n      w = 0;\n    }\n  }\n  if(j > 0) {\n    this.dMultiply(Math.pow(b,j));\n    this.dAddOffset(w,0);\n  }\n  if(mi) BigInteger.ZERO.subTo(this,this);\n}\n\n// (protected) alternate constructor\nfunction bnpFromNumber(a,b,c) {\n  if(\"number\" == typeof b) {\n    // new BigInteger(int,int,RNG)\n    if(a < 2) this.fromInt(1);\n    else {\n      this.fromNumber(a,c);\n      if(!this.testBit(a-1))\t// force MSB set\n        this.bitwiseTo(BigInteger.ONE.shiftLeft(a-1),op_or,this);\n      if(this.isEven()) this.dAddOffset(1,0); // force odd\n      while(!this.isProbablePrime(b)) {\n        this.dAddOffset(2,0);\n        if(this.bitLength() > a) this.subTo(BigInteger.ONE.shiftLeft(a-1),this);\n      }\n    }\n  }\n  else {\n    // new BigInteger(int,RNG)\n    var x = new Array(), t = a&7;\n    x.length = (a>>3)+1;\n    b.nextBytes(x);\n    if(t > 0) x[0] &= ((1<<t)-1); else x[0] = 0;\n    this.fromString(x,256);\n  }\n}\n\n// (public) convert to bigendian byte array\nfunction bnToByteArray() {\n  var this_array = this.array;\n  var i = this.t, r = new Array();\n  r[0] = this.s;\n  var p = BI_DB-(i*BI_DB)%8, d, k = 0;\n  if(i-- > 0) {\n    if(p < BI_DB && (d = this_array[i]>>p) != (this.s&BI_DM)>>p)\n      r[k++] = d|(this.s<<(BI_DB-p));\n    while(i >= 0) {\n      if(p < 8) {\n        d = (this_array[i]&((1<<p)-1))<<(8-p);\n        d |= this_array[--i]>>(p+=BI_DB-8);\n      }\n      else {\n        d = (this_array[i]>>(p-=8))&0xff;\n        if(p <= 0) { p += BI_DB; --i; }\n      }\n      if((d&0x80) != 0) d |= -256;\n      if(k == 0 && (this.s&0x80) != (d&0x80)) ++k;\n      if(k > 0 || d != this.s) r[k++] = d;\n    }\n  }\n  return r;\n}\n\nfunction bnEquals(a) { return(this.compareTo(a)==0); }\nfunction bnMin(a) { return(this.compareTo(a)<0)?this:a; }\nfunction bnMax(a) { return(this.compareTo(a)>0)?this:a; }\n\n// (protected) r = this op a (bitwise)\nfunction bnpBitwiseTo(a,op,r) {\n  var this_array = this.array;\n  var a_array    = a.array;\n  var r_array    = r.array;\n  var i, f, m = Math.min(a.t,this.t);\n  for(i = 0; i < m; ++i) r_array[i] = op(this_array[i],a_array[i]);\n  if(a.t < this.t) {\n    f = a.s&BI_DM;\n    for(i = m; i < this.t; ++i) r_array[i] = op(this_array[i],f);\n    r.t = this.t;\n  }\n  else {\n    f = this.s&BI_DM;\n    for(i = m; i < a.t; ++i) r_array[i] = op(f,a_array[i]);\n    r.t = a.t;\n  }\n  r.s = op(this.s,a.s);\n  r.clamp();\n}\n\n// (public) this & a\nfunction op_and(x,y) { return x&y; }\nfunction bnAnd(a) { var r = nbi(); this.bitwiseTo(a,op_and,r); return r; }\n\n// (public) this | a\nfunction op_or(x,y) { return x|y; }\nfunction bnOr(a) { var r = nbi(); this.bitwiseTo(a,op_or,r); return r; }\n\n// (public) this ^ a\nfunction op_xor(x,y) { return x^y; }\nfunction bnXor(a) { var r = nbi(); this.bitwiseTo(a,op_xor,r); return r; }\n\n// (public) this & ~a\nfunction op_andnot(x,y) { return x&~y; }\nfunction bnAndNot(a) { var r = nbi(); this.bitwiseTo(a,op_andnot,r); return r; }\n\n// (public) ~this\nfunction bnNot() {\n  var this_array = this.array;\n  var r = nbi();\n  var r_array = r.array;\n\n  for(var i = 0; i < this.t; ++i) r_array[i] = BI_DM&~this_array[i];\n  r.t = this.t;\n  r.s = ~this.s;\n  return r;\n}\n\n// (public) this << n\nfunction bnShiftLeft(n) {\n  var r = nbi();\n  if(n < 0) this.rShiftTo(-n,r); else this.lShiftTo(n,r);\n  return r;\n}\n\n// (public) this >> n\nfunction bnShiftRight(n) {\n  var r = nbi();\n  if(n < 0) this.lShiftTo(-n,r); else this.rShiftTo(n,r);\n  return r;\n}\n\n// return index of lowest 1-bit in x, x < 2^31\nfunction lbit(x) {\n  if(x == 0) return -1;\n  var r = 0;\n  if((x&0xffff) == 0) { x >>= 16; r += 16; }\n  if((x&0xff) == 0) { x >>= 8; r += 8; }\n  if((x&0xf) == 0) { x >>= 4; r += 4; }\n  if((x&3) == 0) { x >>= 2; r += 2; }\n  if((x&1) == 0) ++r;\n  return r;\n}\n\n// (public) returns index of lowest 1-bit (or -1 if none)\nfunction bnGetLowestSetBit() {\n  var this_array = this.array;\n  for(var i = 0; i < this.t; ++i)\n    if(this_array[i] != 0) return i*BI_DB+lbit(this_array[i]);\n  if(this.s < 0) return this.t*BI_DB;\n  return -1;\n}\n\n// return number of 1 bits in x\nfunction cbit(x) {\n  var r = 0;\n  while(x != 0) { x &= x-1; ++r; }\n  return r;\n}\n\n// (public) return number of set bits\nfunction bnBitCount() {\n  var r = 0, x = this.s&BI_DM;\n  for(var i = 0; i < this.t; ++i) r += cbit(this_array[i]^x);\n  return r;\n}\n\n// (public) true iff nth bit is set\nfunction bnTestBit(n) {\n  var this_array = this.array;\n  var j = Math.floor(n/BI_DB);\n  if(j >= this.t) return(this.s!=0);\n  return((this_array[j]&(1<<(n%BI_DB)))!=0);\n}\n\n// (protected) this op (1<<n)\nfunction bnpChangeBit(n,op) {\n  var r = BigInteger.ONE.shiftLeft(n);\n  this.bitwiseTo(r,op,r);\n  return r;\n}\n\n// (public) this | (1<<n)\nfunction bnSetBit(n) { return this.changeBit(n,op_or); }\n\n// (public) this & ~(1<<n)\nfunction bnClearBit(n) { return this.changeBit(n,op_andnot); }\n\n// (public) this ^ (1<<n)\nfunction bnFlipBit(n) { return this.changeBit(n,op_xor); }\n\n// (protected) r = this + a\nfunction bnpAddTo(a,r) {\n  var this_array = this.array;\n  var a_array = a.array;\n  var r_array = r.array;\n  var i = 0, c = 0, m = Math.min(a.t,this.t);\n  while(i < m) {\n    c += this_array[i]+a_array[i];\n    r_array[i++] = c&BI_DM;\n    c >>= BI_DB;\n  }\n  if(a.t < this.t) {\n    c += a.s;\n    while(i < this.t) {\n      c += this_array[i];\n      r_array[i++] = c&BI_DM;\n      c >>= BI_DB;\n    }\n    c += this.s;\n  }\n  else {\n    c += this.s;\n    while(i < a.t) {\n      c += a_array[i];\n      r_array[i++] = c&BI_DM;\n      c >>= BI_DB;\n    }\n    c += a.s;\n  }\n  r.s = (c<0)?-1:0;\n  if(c > 0) r_array[i++] = c;\n  else if(c < -1) r_array[i++] = BI_DV+c;\n  r.t = i;\n  r.clamp();\n}\n\n// (public) this + a\nfunction bnAdd(a) { var r = nbi(); this.addTo(a,r); return r; }\n\n// (public) this - a\nfunction bnSubtract(a) { var r = nbi(); this.subTo(a,r); return r; }\n\n// (public) this * a\nfunction bnMultiply(a) { var r = nbi(); this.multiplyTo(a,r); return r; }\n\n// (public) this / a\nfunction bnDivide(a) { var r = nbi(); this.divRemTo(a,r,null); return r; }\n\n// (public) this % a\nfunction bnRemainder(a) { var r = nbi(); this.divRemTo(a,null,r); return r; }\n\n// (public) [this/a,this%a]\nfunction bnDivideAndRemainder(a) {\n  var q = nbi(), r = nbi();\n  this.divRemTo(a,q,r);\n  return new Array(q,r);\n}\n\n// (protected) this *= n, this >= 0, 1 < n < DV\nfunction bnpDMultiply(n) {\n  var this_array = this.array;\n  this_array[this.t] = this.am(0,n-1,this,0,0,this.t);\n  ++this.t;\n  this.clamp();\n}\n\n// (protected) this += n << w words, this >= 0\nfunction bnpDAddOffset(n,w) {\n  var this_array = this.array;\n  while(this.t <= w) this_array[this.t++] = 0;\n  this_array[w] += n;\n  while(this_array[w] >= BI_DV) {\n    this_array[w] -= BI_DV;\n    if(++w >= this.t) this_array[this.t++] = 0;\n    ++this_array[w];\n  }\n}\n\n// A \"null\" reducer\nfunction NullExp() {}\nfunction nNop(x) { return x; }\nfunction nMulTo(x,y,r) { x.multiplyTo(y,r); }\nfunction nSqrTo(x,r) { x.squareTo(r); }\n\nNullExp.prototype.convert = nNop;\nNullExp.prototype.revert = nNop;\nNullExp.prototype.mulTo = nMulTo;\nNullExp.prototype.sqrTo = nSqrTo;\n\n// (public) this^e\nfunction bnPow(e) { return this.exp(e,new NullExp()); }\n\n// (protected) r = lower n words of \"this * a\", a.t <= n\n// \"this\" should be the larger one if appropriate.\nfunction bnpMultiplyLowerTo(a,n,r) {\n  var r_array = r.array;\n  var a_array = a.array;\n  var i = Math.min(this.t+a.t,n);\n  r.s = 0; // assumes a,this >= 0\n  r.t = i;\n  while(i > 0) r_array[--i] = 0;\n  var j;\n  for(j = r.t-this.t; i < j; ++i) r_array[i+this.t] = this.am(0,a_array[i],r,i,0,this.t);\n  for(j = Math.min(a.t,n); i < j; ++i) this.am(0,a_array[i],r,i,0,n-i);\n  r.clamp();\n}\n\n// (protected) r = \"this * a\" without lower n words, n > 0\n// \"this\" should be the larger one if appropriate.\nfunction bnpMultiplyUpperTo(a,n,r) {\n  var r_array = r.array;\n  var a_array = a.array;\n  --n;\n  var i = r.t = this.t+a.t-n;\n  r.s = 0; // assumes a,this >= 0\n  while(--i >= 0) r_array[i] = 0;\n  for(i = Math.max(n-this.t,0); i < a.t; ++i)\n    r_array[this.t+i-n] = this.am(n-i,a_array[i],r,0,0,this.t+i-n);\n  r.clamp();\n  r.drShiftTo(1,r);\n}\n\n// Barrett modular reduction\nfunction Barrett(m) {\n  // setup Barrett\n  this.r2 = nbi();\n  this.q3 = nbi();\n  BigInteger.ONE.dlShiftTo(2*m.t,this.r2);\n  this.mu = this.r2.divide(m);\n  this.m = m;\n}\n\nfunction barrettConvert(x) {\n  if(x.s < 0 || x.t > 2*this.m.t) return x.mod(this.m);\n  else if(x.compareTo(this.m) < 0) return x;\n  else { var r = nbi(); x.copyTo(r); this.reduce(r); return r; }\n}\n\nfunction barrettRevert(x) { return x; }\n\n// x = x mod m (HAC 14.42)\nfunction barrettReduce(x) {\n  x.drShiftTo(this.m.t-1,this.r2);\n  if(x.t > this.m.t+1) { x.t = this.m.t+1; x.clamp(); }\n  this.mu.multiplyUpperTo(this.r2,this.m.t+1,this.q3);\n  this.m.multiplyLowerTo(this.q3,this.m.t+1,this.r2);\n  while(x.compareTo(this.r2) < 0) x.dAddOffset(1,this.m.t+1);\n  x.subTo(this.r2,x);\n  while(x.compareTo(this.m) >= 0) x.subTo(this.m,x);\n}\n\n// r = x^2 mod m; x != r\nfunction barrettSqrTo(x,r) { x.squareTo(r); this.reduce(r); }\n\n// r = x*y mod m; x,y != r\nfunction barrettMulTo(x,y,r) { x.multiplyTo(y,r); this.reduce(r); }\n\nBarrett.prototype.convert = barrettConvert;\nBarrett.prototype.revert = barrettRevert;\nBarrett.prototype.reduce = barrettReduce;\nBarrett.prototype.mulTo = barrettMulTo;\nBarrett.prototype.sqrTo = barrettSqrTo;\n\n// (public) this^e % m (HAC 14.85)\nfunction bnModPow(e,m) {\n  var e_array = e.array;\n  var i = e.bitLength(), k, r = nbv(1), z;\n  if(i <= 0) return r;\n  else if(i < 18) k = 1;\n  else if(i < 48) k = 3;\n  else if(i < 144) k = 4;\n  else if(i < 768) k = 5;\n  else k = 6;\n  if(i < 8)\n    z = new Classic(m);\n  else if(m.isEven())\n    z = new Barrett(m);\n  else\n    z = new Montgomery(m);\n\n  // precomputation\n  var g = new Array(), n = 3, k1 = k-1, km = (1<<k)-1;\n  g[1] = z.convert(this);\n  if(k > 1) {\n    var g2 = nbi();\n    z.sqrTo(g[1],g2);\n    while(n <= km) {\n      g[n] = nbi();\n      z.mulTo(g2,g[n-2],g[n]);\n      n += 2;\n    }\n  }\n\n  var j = e.t-1, w, is1 = true, r2 = nbi(), t;\n  i = nbits(e_array[j])-1;\n  while(j >= 0) {\n    if(i >= k1) w = (e_array[j]>>(i-k1))&km;\n    else {\n      w = (e_array[j]&((1<<(i+1))-1))<<(k1-i);\n      if(j > 0) w |= e_array[j-1]>>(BI_DB+i-k1);\n    }\n\n    n = k;\n    while((w&1) == 0) { w >>= 1; --n; }\n    if((i -= n) < 0) { i += BI_DB; --j; }\n    if(is1) {\t// ret == 1, don't bother squaring or multiplying it\n      g[w].copyTo(r);\n      is1 = false;\n    }\n    else {\n      while(n > 1) { z.sqrTo(r,r2); z.sqrTo(r2,r); n -= 2; }\n      if(n > 0) z.sqrTo(r,r2); else { t = r; r = r2; r2 = t; }\n      z.mulTo(r2,g[w],r);\n    }\n\n    while(j >= 0 && (e_array[j]&(1<<i)) == 0) {\n      z.sqrTo(r,r2); t = r; r = r2; r2 = t;\n      if(--i < 0) { i = BI_DB-1; --j; }\n    }\n  }\n  return z.revert(r);\n}\n\n// (public) gcd(this,a) (HAC 14.54)\nfunction bnGCD(a) {\n  var x = (this.s<0)?this.negate():this.clone();\n  var y = (a.s<0)?a.negate():a.clone();\n  if(x.compareTo(y) < 0) { var t = x; x = y; y = t; }\n  var i = x.getLowestSetBit(), g = y.getLowestSetBit();\n  if(g < 0) return x;\n  if(i < g) g = i;\n  if(g > 0) {\n    x.rShiftTo(g,x);\n    y.rShiftTo(g,y);\n  }\n  while(x.signum() > 0) {\n    if((i = x.getLowestSetBit()) > 0) x.rShiftTo(i,x);\n    if((i = y.getLowestSetBit()) > 0) y.rShiftTo(i,y);\n    if(x.compareTo(y) >= 0) {\n      x.subTo(y,x);\n      x.rShiftTo(1,x);\n    }\n    else {\n      y.subTo(x,y);\n      y.rShiftTo(1,y);\n    }\n  }\n  if(g > 0) y.lShiftTo(g,y);\n  return y;\n}\n\n// (protected) this % n, n < 2^26\nfunction bnpModInt(n) {\n  var this_array = this.array;\n  if(n <= 0) return 0;\n  var d = BI_DV%n, r = (this.s<0)?n-1:0;\n  if(this.t > 0)\n    if(d == 0) r = this_array[0]%n;\n    else for(var i = this.t-1; i >= 0; --i) r = (d*r+this_array[i])%n;\n  return r;\n}\n\n// (public) 1/this % m (HAC 14.61)\nfunction bnModInverse(m) {\n  var ac = m.isEven();\n  if((this.isEven() && ac) || m.signum() == 0) return BigInteger.ZERO;\n  var u = m.clone(), v = this.clone();\n  var a = nbv(1), b = nbv(0), c = nbv(0), d = nbv(1);\n  while(u.signum() != 0) {\n    while(u.isEven()) {\n      u.rShiftTo(1,u);\n      if(ac) {\n        if(!a.isEven() || !b.isEven()) { a.addTo(this,a); b.subTo(m,b); }\n        a.rShiftTo(1,a);\n      }\n      else if(!b.isEven()) b.subTo(m,b);\n      b.rShiftTo(1,b);\n    }\n    while(v.isEven()) {\n      v.rShiftTo(1,v);\n      if(ac) {\n        if(!c.isEven() || !d.isEven()) { c.addTo(this,c); d.subTo(m,d); }\n        c.rShiftTo(1,c);\n      }\n      else if(!d.isEven()) d.subTo(m,d);\n      d.rShiftTo(1,d);\n    }\n    if(u.compareTo(v) >= 0) {\n      u.subTo(v,u);\n      if(ac) a.subTo(c,a);\n      b.subTo(d,b);\n    }\n    else {\n      v.subTo(u,v);\n      if(ac) c.subTo(a,c);\n      d.subTo(b,d);\n    }\n  }\n  if(v.compareTo(BigInteger.ONE) != 0) return BigInteger.ZERO;\n  if(d.compareTo(m) >= 0) return d.subtract(m);\n  if(d.signum() < 0) d.addTo(m,d); else return d;\n  if(d.signum() < 0) return d.add(m); else return d;\n}\n\nvar lowprimes = [2,3,5,7,11,13,17,19,23,29,31,37,41,43,47,53,59,61,67,71,73,79,83,89,97,101,103,107,109,113,127,131,137,139,149,151,157,163,167,173,179,181,191,193,197,199,211,223,227,229,233,239,241,251,257,263,269,271,277,281,283,293,307,311,313,317,331,337,347,349,353,359,367,373,379,383,389,397,401,409,419,421,431,433,439,443,449,457,461,463,467,479,487,491,499,503,509];\nvar lplim = (1<<26)/lowprimes[lowprimes.length-1];\n\n// (public) test primality with certainty >= 1-.5^t\nfunction bnIsProbablePrime(t) {\n  var i, x = this.abs();\n  var x_array = x.array;\n  if(x.t == 1 && x_array[0] <= lowprimes[lowprimes.length-1]) {\n    for(i = 0; i < lowprimes.length; ++i)\n      if(x_array[0] == lowprimes[i]) return true;\n    return false;\n  }\n  if(x.isEven()) return false;\n  i = 1;\n  while(i < lowprimes.length) {\n    var m = lowprimes[i], j = i+1;\n    while(j < lowprimes.length && m < lplim) m *= lowprimes[j++];\n    m = x.modInt(m);\n    while(i < j) if(m%lowprimes[i++] == 0) return false;\n  }\n  return x.millerRabin(t);\n}\n\n// (protected) true if probably prime (HAC 4.24, Miller-Rabin)\nfunction bnpMillerRabin(t) {\n  var n1 = this.subtract(BigInteger.ONE);\n  var k = n1.getLowestSetBit();\n  if(k <= 0) return false;\n  var r = n1.shiftRight(k);\n  t = (t+1)>>1;\n  if(t > lowprimes.length) t = lowprimes.length;\n  var a = nbi();\n  for(var i = 0; i < t; ++i) {\n    a.fromInt(lowprimes[i]);\n    var y = a.modPow(r,this);\n    if(y.compareTo(BigInteger.ONE) != 0 && y.compareTo(n1) != 0) {\n      var j = 1;\n      while(j++ < k && y.compareTo(n1) != 0) {\n        y = y.modPowInt(2,this);\n        if(y.compareTo(BigInteger.ONE) == 0) return false;\n      }\n      if(y.compareTo(n1) != 0) return false;\n    }\n  }\n  return true;\n}\n\n// protected\nBigInteger.prototype.chunkSize = bnpChunkSize;\nBigInteger.prototype.toRadix = bnpToRadix;\nBigInteger.prototype.fromRadix = bnpFromRadix;\nBigInteger.prototype.fromNumber = bnpFromNumber;\nBigInteger.prototype.bitwiseTo = bnpBitwiseTo;\nBigInteger.prototype.changeBit = bnpChangeBit;\nBigInteger.prototype.addTo = bnpAddTo;\nBigInteger.prototype.dMultiply = bnpDMultiply;\nBigInteger.prototype.dAddOffset = bnpDAddOffset;\nBigInteger.prototype.multiplyLowerTo = bnpMultiplyLowerTo;\nBigInteger.prototype.multiplyUpperTo = bnpMultiplyUpperTo;\nBigInteger.prototype.modInt = bnpModInt;\nBigInteger.prototype.millerRabin = bnpMillerRabin;\n\n// public\nBigInteger.prototype.clone = bnClone;\nBigInteger.prototype.intValue = bnIntValue;\nBigInteger.prototype.byteValue = bnByteValue;\nBigInteger.prototype.shortValue = bnShortValue;\nBigInteger.prototype.signum = bnSigNum;\nBigInteger.prototype.toByteArray = bnToByteArray;\nBigInteger.prototype.equals = bnEquals;\nBigInteger.prototype.min = bnMin;\nBigInteger.prototype.max = bnMax;\nBigInteger.prototype.and = bnAnd;\nBigInteger.prototype.or = bnOr;\nBigInteger.prototype.xor = bnXor;\nBigInteger.prototype.andNot = bnAndNot;\nBigInteger.prototype.not = bnNot;\nBigInteger.prototype.shiftLeft = bnShiftLeft;\nBigInteger.prototype.shiftRight = bnShiftRight;\nBigInteger.prototype.getLowestSetBit = bnGetLowestSetBit;\nBigInteger.prototype.bitCount = bnBitCount;\nBigInteger.prototype.testBit = bnTestBit;\nBigInteger.prototype.setBit = bnSetBit;\nBigInteger.prototype.clearBit = bnClearBit;\nBigInteger.prototype.flipBit = bnFlipBit;\nBigInteger.prototype.add = bnAdd;\nBigInteger.prototype.subtract = bnSubtract;\nBigInteger.prototype.multiply = bnMultiply;\nBigInteger.prototype.divide = bnDivide;\nBigInteger.prototype.remainder = bnRemainder;\nBigInteger.prototype.divideAndRemainder = bnDivideAndRemainder;\nBigInteger.prototype.modPow = bnModPow;\nBigInteger.prototype.modInverse = bnModInverse;\nBigInteger.prototype.pow = bnPow;\nBigInteger.prototype.gcd = bnGCD;\nBigInteger.prototype.isProbablePrime = bnIsProbablePrime;\n\n// BigInteger interfaces not implemented in jsbn:\n\n// BigInteger(int signum, byte[] magnitude)\n// double doubleValue()\n// float floatValue()\n// int hashCode()\n// long longValue()\n// static BigInteger valueOf(long val)\n// prng4.js - uses Arcfour as a PRNG\n\nfunction Arcfour() {\n  this.i = 0;\n  this.j = 0;\n  this.S = new Array();\n}\n\n// Initialize arcfour context from key, an array of ints, each from [0..255]\nfunction ARC4init(key) {\n  var i, j, t;\n  for(i = 0; i < 256; ++i)\n    this.S[i] = i;\n  j = 0;\n  for(i = 0; i < 256; ++i) {\n    j = (j + this.S[i] + key[i % key.length]) & 255;\n    t = this.S[i];\n    this.S[i] = this.S[j];\n    this.S[j] = t;\n  }\n  this.i = 0;\n  this.j = 0;\n}\n\nfunction ARC4next() {\n  var t;\n  this.i = (this.i + 1) & 255;\n  this.j = (this.j + this.S[this.i]) & 255;\n  t = this.S[this.i];\n  this.S[this.i] = this.S[this.j];\n  this.S[this.j] = t;\n  return this.S[(t + this.S[this.i]) & 255];\n}\n\nArcfour.prototype.init = ARC4init;\nArcfour.prototype.next = ARC4next;\n\n// Plug in your RNG constructor here\nfunction prng_newstate() {\n  return new Arcfour();\n}\n\n// Pool size must be a multiple of 4 and greater than 32.\n// An array of bytes the size of the pool will be passed to init()\nvar rng_psize = 256;\n// Random number generator - requires a PRNG backend, e.g. prng4.js\n\n// For best results, put code like\n// <body onClick='rng_seed_time();' onKeyPress='rng_seed_time();'>\n// in your main HTML document.\n\nvar rng_state;\nvar rng_pool;\nvar rng_pptr;\n\n// Mix in a 32-bit integer into the pool\nfunction rng_seed_int(x) {\n  rng_pool[rng_pptr++] ^= x & 255;\n  rng_pool[rng_pptr++] ^= (x >> 8) & 255;\n  rng_pool[rng_pptr++] ^= (x >> 16) & 255;\n  rng_pool[rng_pptr++] ^= (x >> 24) & 255;\n  if(rng_pptr >= rng_psize) rng_pptr -= rng_psize;\n}\n\n// Mix in the current time (w/milliseconds) into the pool\nfunction rng_seed_time() {\n  // Use pre-computed date to avoid making the benchmark\n  // results dependent on the current date.\n  rng_seed_int(1122926989487);\n}\n\n// Initialize the pool with junk if needed.\nif(rng_pool == null) {\n  rng_pool = new Array();\n  rng_pptr = 0;\n  var t;\n  while(rng_pptr < rng_psize) {  // extract some randomness from Math.random()\n    t = Math.floor(65536 * Math.random());\n    rng_pool[rng_pptr++] = t >>> 8;\n    rng_pool[rng_pptr++] = t & 255;\n  }\n  rng_pptr = 0;\n  rng_seed_time();\n  //rng_seed_int(window.screenX);\n  //rng_seed_int(window.screenY);\n}\n\nfunction rng_get_byte() {\n  if(rng_state == null) {\n    rng_seed_time();\n    rng_state = prng_newstate();\n    rng_state.init(rng_pool);\n    for(rng_pptr = 0; rng_pptr < rng_pool.length; ++rng_pptr)\n      rng_pool[rng_pptr] = 0;\n    rng_pptr = 0;\n    //rng_pool = null;\n  }\n  // TODO: allow reseeding after first request\n  return rng_state.next();\n}\n\nfunction rng_get_bytes(ba) {\n  var i;\n  for(i = 0; i < ba.length; ++i) ba[i] = rng_get_byte();\n}\n\nfunction SecureRandom() {}\n\nSecureRandom.prototype.nextBytes = rng_get_bytes;\n// Depends on jsbn.js and rng.js\n\n// convert a (hex) string to a bignum object\nfunction parseBigInt(str,r) {\n  return new BigInteger(str,r);\n}\n\nfunction linebrk(s,n) {\n  var ret = \"\";\n  var i = 0;\n  while(i + n < s.length) {\n    ret += s.substring(i,i+n) + \"\\n\";\n    i += n;\n  }\n  return ret + s.substring(i,s.length);\n}\n\nfunction byte2Hex(b) {\n  if(b < 0x10)\n    return \"0\" + b.toString(16);\n  else\n    return b.toString(16);\n}\n\n// PKCS#1 (type 2, random) pad input string s to n bytes, and return a bigint\nfunction pkcs1pad2(s,n) {\n  if(n < s.length + 11) {\n    alert(\"Message too long for RSA\");\n    return null;\n  }\n  var ba = new Array();\n  var i = s.length - 1;\n  while(i >= 0 && n > 0) ba[--n] = s.charCodeAt(i--);\n  ba[--n] = 0;\n  var rng = new SecureRandom();\n  var x = new Array();\n  while(n > 2) { // random non-zero pad\n    x[0] = 0;\n    while(x[0] == 0) rng.nextBytes(x);\n    ba[--n] = x[0];\n  }\n  ba[--n] = 2;\n  ba[--n] = 0;\n  return new BigInteger(ba);\n}\n\n// \"empty\" RSA key constructor\nfunction RSAKey() {\n  this.n = null;\n  this.e = 0;\n  this.d = null;\n  this.p = null;\n  this.q = null;\n  this.dmp1 = null;\n  this.dmq1 = null;\n  this.coeff = null;\n}\n\n// Set the public key fields N and e from hex strings\nfunction RSASetPublic(N,E) {\n  if(N != null && E != null && N.length > 0 && E.length > 0) {\n    this.n = parseBigInt(N,16);\n    this.e = parseInt(E,16);\n  }\n  else\n    alert(\"Invalid RSA public key\");\n}\n\n// Perform raw public operation on \"x\": return x^e (mod n)\nfunction RSADoPublic(x) {\n  return x.modPowInt(this.e, this.n);\n}\n\n// Return the PKCS#1 RSA encryption of \"text\" as an even-length hex string\nfunction RSAEncrypt(text) {\n  var m = pkcs1pad2(text,(this.n.bitLength()+7)>>3);\n  if(m == null) return null;\n  var c = this.doPublic(m);\n  if(c == null) return null;\n  var h = c.toString(16);\n  if((h.length & 1) == 0) return h; else return \"0\" + h;\n}\n\n// Return the PKCS#1 RSA encryption of \"text\" as a Base64-encoded string\n//function RSAEncryptB64(text) {\n//  var h = this.encrypt(text);\n//  if(h) return hex2b64(h); else return null;\n//}\n\n// protected\nRSAKey.prototype.doPublic = RSADoPublic;\n\n// public\nRSAKey.prototype.setPublic = RSASetPublic;\nRSAKey.prototype.encrypt = RSAEncrypt;\n//RSAKey.prototype.encrypt_b64 = RSAEncryptB64;\n// Depends on rsa.js and jsbn2.js\n\n// Undo PKCS#1 (type 2, random) padding and, if valid, return the plaintext\nfunction pkcs1unpad2(d,n) {\n  var b = d.toByteArray();\n  var i = 0;\n  while(i < b.length && b[i] == 0) ++i;\n  if(b.length-i != n-1 || b[i] != 2)\n    return null;\n  ++i;\n  while(b[i] != 0)\n    if(++i >= b.length) return null;\n  var ret = \"\";\n  while(++i < b.length)\n    ret += String.fromCharCode(b[i]);\n  return ret;\n}\n\n// Set the private key fields N, e, and d from hex strings\nfunction RSASetPrivate(N,E,D) {\n  if(N != null && E != null && N.length > 0 && E.length > 0) {\n    this.n = parseBigInt(N,16);\n    this.e = parseInt(E,16);\n    this.d = parseBigInt(D,16);\n  }\n  else\n    alert(\"Invalid RSA private key\");\n}\n\n// Set the private key fields N, e, d and CRT params from hex strings\nfunction RSASetPrivateEx(N,E,D,P,Q,DP,DQ,C) {\n  if(N != null && E != null && N.length > 0 && E.length > 0) {\n    this.n = parseBigInt(N,16);\n    this.e = parseInt(E,16);\n    this.d = parseBigInt(D,16);\n    this.p = parseBigInt(P,16);\n    this.q = parseBigInt(Q,16);\n    this.dmp1 = parseBigInt(DP,16);\n    this.dmq1 = parseBigInt(DQ,16);\n    this.coeff = parseBigInt(C,16);\n  }\n  else\n    alert(\"Invalid RSA private key\");\n}\n\n// Generate a new random private key B bits long, using public expt E\nfunction RSAGenerate(B,E) {\n  var rng = new SecureRandom();\n  var qs = B>>1;\n  this.e = parseInt(E,16);\n  var ee = new BigInteger(E,16);\n  for(;;) {\n    for(;;) {\n      this.p = new BigInteger(B-qs,1,rng);\n      if(this.p.subtract(BigInteger.ONE).gcd(ee).compareTo(BigInteger.ONE) == 0 && this.p.isProbablePrime(10)) break;\n    }\n    for(;;) {\n      this.q = new BigInteger(qs,1,rng);\n      if(this.q.subtract(BigInteger.ONE).gcd(ee).compareTo(BigInteger.ONE) == 0 && this.q.isProbablePrime(10)) break;\n    }\n    if(this.p.compareTo(this.q) <= 0) {\n      var t = this.p;\n      this.p = this.q;\n      this.q = t;\n    }\n    var p1 = this.p.subtract(BigInteger.ONE);\n    var q1 = this.q.subtract(BigInteger.ONE);\n    var phi = p1.multiply(q1);\n    if(phi.gcd(ee).compareTo(BigInteger.ONE) == 0) {\n      this.n = this.p.multiply(this.q);\n      this.d = ee.modInverse(phi);\n      this.dmp1 = this.d.mod(p1);\n      this.dmq1 = this.d.mod(q1);\n      this.coeff = this.q.modInverse(this.p);\n      break;\n    }\n  }\n}\n\n// Perform raw private operation on \"x\": return x^d (mod n)\nfunction RSADoPrivate(x) {\n  if(this.p == null || this.q == null)\n    return x.modPow(this.d, this.n);\n\n  // TODO: re-calculate any missing CRT params\n  var xp = x.mod(this.p).modPow(this.dmp1, this.p);\n  var xq = x.mod(this.q).modPow(this.dmq1, this.q);\n\n  while(xp.compareTo(xq) < 0)\n    xp = xp.add(this.p);\n  return xp.subtract(xq).multiply(this.coeff).mod(this.p).multiply(this.q).add(xq);\n}\n\n// Return the PKCS#1 RSA decryption of \"ctext\".\n// \"ctext\" is an even-length hex string and the output is a plain string.\nfunction RSADecrypt(ctext) {\n  var c = parseBigInt(ctext, 16);\n  var m = this.doPrivate(c);\n  if(m == null) return null;\n  return pkcs1unpad2(m, (this.n.bitLength()+7)>>3);\n}\n\n// Return the PKCS#1 RSA decryption of \"ctext\".\n// \"ctext\" is a Base64-encoded string and the output is a plain string.\n//function RSAB64Decrypt(ctext) {\n//  var h = b64tohex(ctext);\n//  if(h) return this.decrypt(h); else return null;\n//}\n\n// protected\nRSAKey.prototype.doPrivate = RSADoPrivate;\n\n// public\nRSAKey.prototype.setPrivate = RSASetPrivate;\nRSAKey.prototype.setPrivateEx = RSASetPrivateEx;\nRSAKey.prototype.generate = RSAGenerate;\nRSAKey.prototype.decrypt = RSADecrypt;\n//RSAKey.prototype.b64_decrypt = RSAB64Decrypt;\n\n\nnValue=\"a5261939975948bb7a58dffe5ff54e65f0498f9175f5a09288810b8975871e99af3b5dd94057b0fc07535f5f97444504fa35169d461d0d30cf0192e307727c065168c788771c561a9400fb49175e9e6aa4e23fe11af69e9412dd23b0cb6684c4c2429bce139e848ab26d0829073351f4acd36074eafd036a5eb83359d2a698d3\";\neValue=\"10001\";\ndValue=\"8e9912f6d3645894e8d38cb58c0db81ff516cf4c7e5a14c7f1eddb1459d2cded4d8d293fc97aee6aefb861859c8b6a3d1dfe710463e1f9ddc72048c09751971c4a580aa51eb523357a3cc48d31cfad1d4a165066ed92d4748fb6571211da5cb14bc11b6e2df7c1a559e6d5ac1cd5c94703a22891464fba23d0d965086277a161\";\npValue=\"d090ce58a92c75233a6486cb0a9209bf3583b64f540c76f5294bb97d285eed33aec220bde14b2417951178ac152ceab6da7090905b478195498b352048f15e7d\";\nqValue=\"cab575dc652bb66df15a0359609d51d1db184750c00c6698b90ef3465c99655103edbf0d54c56aec0ce3c4d22592338092a126a0cc49f65a4a30d222b411e58f\";\ndmp1Value=\"1a24bca8e273df2f0e47c199bbf678604e7df7215480c77c8db39f49b000ce2cf7500038acfff5433b7d582a01f1826e6f4d42e1c57f5e1fef7b12aabc59fd25\";\ndmq1Value=\"3d06982efbbe47339e1f6d36b1216b8a741d410b0c662f54f7118b27b9a4ec9d914337eb39841d8666f3034408cf94f5b62f11c402fc994fe15a05493150d9fd\";\ncoeffValue=\"3a3e731acd8960b7ff9eb81a7ff93bd1cfa74cbd56987db58b4594fb09c09084db1734c8143f98b602b981aaa9243ca28deb69b5b280ee8dcee0fd2625e53250\";\n\nsetupEngine(am3, 28);\n\nvar TEXT = \"The quick brown fox jumped over the extremely lazy frog! \" +\n    \"Now is the time for all good men to come to the party.\";\nvar encrypted;\n\nfunction encrypt() {\n  var RSA = new RSAKey();\n  RSA.setPublic(nValue, eValue);\n  RSA.setPrivateEx(nValue, eValue, dValue, pValue, qValue, dmp1Value, dmq1Value, coeffValue);\n  encrypted = RSA.encrypt(TEXT);\n}\n\nfunction decrypt() {\n  var RSA = new RSAKey();\n  RSA.setPublic(nValue, eValue);\n  RSA.setPrivateEx(nValue, eValue, dValue, pValue, qValue, dmp1Value, dmq1Value, coeffValue);\n  var decrypted = RSA.decrypt(encrypted);\n  if (decrypted != TEXT) {\n    throw new Error(\"Crypto operation failed\");\n  }\n}\n\nclass Benchmark {\n    runIteration() {\n        encrypt();\n        decrypt();\n    }\n}\n\n"};
var readFile = function (name) {
    const normalized = String(name).replaceAll("\\", "/");
    if (!Object.prototype.hasOwnProperty.call(__jetstreamResources, normalized))
        throw new Error("JetStream resource not embedded: " + normalized);
    return __jetstreamResources[normalized];
};
var read = function (name, mode) {
    const text = readFile(name);
    if (mode !== "binary")
        return text;
    const bytes = [];
    for (let i = 0; i < text.length; i++)
        bytes.push(text.charCodeAt(i) & 0xff);
    return bytes;
};

"use strict";

/*
 * Copyright (C) 2018-2024 Apple Inc. All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 * 1. Redistributions of source code must retain the above copyright
 *    notice, this list of conditions and the following disclaimer.
 * 2. Redistributions in binary form must reproduce the above copyright
 *    notice, this list of conditions and the following disclaimer in the
 *    documentation and/or other materials provided with the distribution.
 *
 * THIS SOFTWARE IS PROVIDED BY APPLE INC. AND ITS CONTRIBUTORS ``AS IS''
 * AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO,
 * THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR
 * PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL APPLE INC. OR ITS CONTRIBUTORS
 * BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
 * CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
 * SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
 * INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
 * CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
 * ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF
 * THE POSSIBILITY OF SUCH DAMAGE.
*/

const measureTotalTimeAsSubtest = false; // Once we move to preloading all resources, it would be good to turn this on.

const defaultIterationCount = 120;
const defaultWorstCaseCount = 4;

if (!JetStreamParams.prefetchResources && isInBrowser) {
    console.warn("Disabling resource prefetching! All compressed files must have been decompressed using `npm run decompress`");
}

if (JetStreamParams.forceGC && typeof globalThis.gc === "undefined") {
    console.warn("Force-gc is set, but globalThis.gc() is not available.");
}

if (!isInBrowser && JetStreamParams.prefetchResources) {
    // Use the wasm compiled zlib as a polyfill when decompression stream is
    // not available in JS shells.
    load("./wasm/zlib/shell.js");

    // Load a polyfill for TextEncoder/TextDecoder in shells. Used when
    // decompressing a prefetched resource and converting it to text.
    load("./utils/polyfills/fast-text-encoding/1.0.3/text.js");
}

// Used for the promise representing the current benchmark run.
var currentResolve = null;
var currentReject = null;

function displayCategoryScores() {
    document.body.classList.add("details");
}


if (isInBrowser) {
    document.onkeydown = (keyboardEvent) => {
        const key = keyboardEvent.key;
        if (key === "d" || key === "D")
            displayCategoryScores();
    };
}

function sum(values) {
    console.assert(values instanceof Array);
    let sum = 0;
    for (let x of values)
        sum += x;
    return sum;
}

function mean(values) {
    const totalSum = sum(values)
    return totalSum / values.length;
}

function geomeanScore(values) {
    console.assert(values instanceof Array);
    let product = 1;
    for (let x of values)
        product *= x;
    const score = product ** (1 / values.length);
    // Allow 0 for uninitialized subScores().
    console.assert(score >= 0, `Got invalid score: ${score}`)
    return score;
}

function toScore(timeValue) {
    return 5000 / Math.max(timeValue, 1);
}

function toTimeValue(score) {
    return 5000 / score;
}

function updateUI() {
    return new Promise((resolve) => {
        if (isInBrowser)
            requestAnimationFrame(() => setTimeout(resolve, 0));
        else
            resolve();
    });
}

function uiFriendlyNumber(num) {
    if (Number.isInteger(num))
        return num;
    return num.toFixed(2);
}

function uiFriendlyScore(num) {
    return uiFriendlyNumber(num);
}

function uiFriendlyDuration(time) {
    return `${time.toFixed(2)} ms`;
}

const LABEL_PADDING = 45;
function shellFriendlyLabel(label) {
    return `${label}`.padEnd(LABEL_PADDING);
}

const VALUE_PADDING = 11;
function shellFriendlyDuration(time) {
    return `${uiFriendlyDuration(time)} `.padStart(VALUE_PADDING);
}

function shellFriendlyScore(time) {
    return `${uiFriendlyScore(time)} pts`.padStart(VALUE_PADDING);
}


// Files can be zlib compressed to reduce the size of the JetStream source code.
// We don't use http compression because we support running from the shell and
// don't want to require a complicated server setup.
//
// zlib was chosen because we already have it in tree for the wasm-zlib test.
function isCompressed(name) {
    return name.endsWith(".z");
}

function uncompressedName(name) {
    console.assert(isCompressed(name));
    return name.slice(0, -2);
}

// TODO: Cleanup / remove / merge. This is only used for caching loads in the
// non-browser setting. In the browser we use exclusively `loadCache`,
// `loadBlob`, `doLoadBlob`, `prefetchResourcesForBrowser` etc., see below.
class ShellFileLoader {
    constructor() {
        this.requests = new Map;
    }

    // Cache / memoize previously read files, because some workloads
    // share common code.
    load(url) {
        console.assert(!isInBrowser);

        let compressed = isCompressed(url);
        if (compressed && !JetStreamParams.prefetchResources) {
            url = uncompressedName(url);
        }

        // If we aren't supposed to prefetch this then return code snippet that will load the url on-demand.
        if (!JetStreamParams.prefetchResources)
            return readFile(url);

        if (this.requests.has(url)) {
            return this.requests.get(url);
        }

        let contents;
        if (compressed) {
            const compressedBytes = new Int8Array(read(url, "binary"));
            const decompressedBytes = zlib.decompress(compressedBytes);
            contents = new TextDecoder().decode(decompressedBytes);
        } else {
            contents = readFile(url);
        }
        this.requests.set(url, contents);
        return contents;
    }
};


class BrowserFileLoader {

    constructor() {
        // TODO: Cleanup / remove / merge `blobDataCache` and `loadCache` vs.
        // the global `fileLoader` cache.
        this.blobDataCache = { __proto__ : null };
        this.loadCache = { __proto__ : null };
    }

    async doLoadBlob(resource) {
        const blobData = this.blobDataCache[resource];

        const compressed = isCompressed(resource);
        if (compressed && !JetStreamParams.prefetchResources) {
            resource = uncompressedName(resource);
        }

        // If we aren't supposed to prefetch this then set the blobURL to just
        // be the resource URL.
        if (!JetStreamParams.prefetchResources) {
            blobData.blobURL = resource;
            return blobData;
        }

        let response;
        let tries = 3;
        while (tries--) {
            let hasError = false;
            try {
                response = await fetch(resource, { cache: "no-store" });
            } catch (e) {
                hasError = true;
            }
            if (!hasError && response.ok)
                break;
            if (tries)
                continue;
            throw new Error("Fetch failed");
        }

        // If we need to decompress this, then run it through a decompression
        // stream.
        if (compressed) {
            const stream = response.body.pipeThrough(new DecompressionStream("deflate"))
            response = new Response(stream);
        }

        let blob = await response.blob();
        blobData.blob = blob;
        blobData.blobURL = URL.createObjectURL(blob);
        return blobData;
    }

    async loadBlob(type, prop, resource, incrementRefCount = true) {
        let blobData = this.blobDataCache[resource];
        if (!blobData) {
            blobData = {
                type: type,
                prop: prop,
                resource: resource,
                blob: null,
                blobURL: null,
                refCount: 0
            };
            this.blobDataCache[resource] = blobData;
        }

        if (incrementRefCount)
            blobData.refCount++;

        let promise = this.loadCache[resource];
        if (promise)
            return promise;

        promise = this.doLoadBlob(resource);
        this.loadCache[resource] = promise;
        return promise;
    }

    async retryPrefetchResource(type, prop, file) {
        console.assert(isInBrowser);

        const counter = JetStream.counter;
        const blobData = this.blobDataCache[file];
        if (blobData.blob) {
            // The same preload blob may be used by multiple subtests. Though the blob is already loaded,
            // we still need to check if this subtest failed to load it before. If so, handle accordingly.
            if (type == "preload") {
                if (this.failedPreloads && this.failedPreloads[blobData.prop]) {
                    this.failedPreloads[blobData.prop] = false;
                    this._preloadBlobData.push({ name: blobData.prop, resource: blobData.resource, blobURLOrPath: blobData.blobURL });
                    counter.failedPreloadResources--;
                }
            }
            return !counter.failedPreloadResources && counter.loadedResources == counter.totalResources;
        }

        // Retry fetching the resource.
        this.loadCache[file] = null;
        await this.loadBlob(type, prop, file, false).then((blobData) => {
            if (!globalThis.allIsGood)
                return;
            if (blobData.type == "preload")
                this._preloadBlobData.push({ name: blobData.prop, resource: blobData.resource, blobURLOrPath: blobData.blobURL });
            this.updateCounter();
        });

        if (!blobData.blob) {
            globalThis.allIsGood = false;
            throw new Error("Fetch failed");
        }

        return !counter.failedPreloadResources && counter.loadedResources == counter.totalResources;
    }

    free(files) {
        for (const file of files) {
            const blobData = this.blobDataCache[file];
            // If we didn't prefetch this resource, then no need to free it
            if (!blobData.blob) {
                continue
            }
            blobData.refCount--;
            if (!blobData.refCount)
                this.blobDataCache[file] = undefined;
        }
    }
}

const browserFileLoader = new BrowserFileLoader();
const shellFileLoader = new ShellFileLoader();

class Driver {
    constructor(benchmarks) {
        this.isReady = false;
        this.isDone = false;
        this.errors = [];
        // Make benchmark list unique and sort it.
        this.benchmarks = Array.from(new Set(benchmarks));
        this.benchmarks.sort((a, b) => a.name.toLowerCase() < b.name.toLowerCase() ? 1 : -1);
        console.assert(this.benchmarks.length, "No benchmarks selected");
        this.counter = { };
        this.counter.loadedResources = 0;
        this.counter.totalResources = 0;
        this.counter.failedPreloadResources = 0;
    }

    async start() {
        let statusElement = false;
        if (isInBrowser) {
            statusElement = document.getElementById("status");
            statusElement.innerHTML = `<label>Running...</label>`;
        } else if (!JetStreamParams.dumpJSONResults)
            console.log("Starting JetStream3");

        performance.mark("update-ui-start");
        const start = performance.now();
        for (const benchmark of this.benchmarks) {
            performance.mark("update-ui-start");
            benchmark.updateUIBeforeRun();
            await updateUI();
            performance.measure("runner update-ui", "update-ui-start");

            try {
                await benchmark.run();
            } catch(e) {
                this.reportError(benchmark, e);
                throw e;
            }

            performance.mark("update-ui-start");
            benchmark.updateUIAfterRun();
            performance.measure("runner update-ui", "update-ui-start");

            if (isInBrowser) {
                browserFileLoader.free(benchmark.files);
            }
        }

        const totalTime = performance.now() - start;
        if (measureTotalTimeAsSubtest) {
            if (isInBrowser)
                document.getElementById("benchmark-total-time-score").innerHTML = uiFriendlyNumber(totalTime);
            else if (!JetStreamParams.dumpJSONResults)
                console.log("Total-Time:", uiFriendlyNumber(totalTime));
            allScores.push(totalTime);
        }

        const allScores = [];
        for (const benchmark of this.benchmarks) {
            const score = benchmark.score;
            console.assert(score > 0, `Invalid ${benchmark.name} score: ${score}`);
            allScores.push(score);
        }

        const categoryScores = new Map();
        const categoryTimes = new Map();
        for (const benchmark of this.benchmarks) {
            for (let category of Object.keys(benchmark.subScores()))
                categoryScores.set(category, []);
            for (let category of Object.keys(benchmark.subTimes()))
                categoryTimes.set(category, []);
        }

        for (const benchmark of this.benchmarks) {
            for (let [category, value] of Object.entries(benchmark.subScores())) {
                const arr = categoryScores.get(category);
                console.assert(value > 0, `Invalid ${benchmark.name} ${category} score: ${value}`);
                arr.push(value);
            }
            for (let [category, value] of Object.entries(benchmark.subTimes())) {
                const arr = categoryTimes.get(category);
                console.assert(value > 0, `Invalid ${benchmark.name} ${category} time: ${value}`);
                arr.push(value);
            }
        }

        const overallScore = geomeanScore(allScores);
        console.assert(overallScore > 0, `Invalid total score: ${overallScore}`);

        if (isInBrowser) {
            let summaryHtml = `<div class="score">${uiFriendlyScore(overallScore)}</div>
                    <label>Score</label>`;
            summaryHtml += `<div class="benchmark benchmark-done">`;
            for (let [category, scores] of categoryScores) {
                summaryHtml += `<span class="result detail">
                                    <span>${uiFriendlyScore(geomeanScore(scores))}</span>
                                    <label>${category}</label>
                                </span>`;
            }
            summaryHtml += "<br/>";
            for (let [category, times] of categoryTimes) {
                summaryHtml += `<span class="result detail">
                                    <span>${uiFriendlyDuration(geomeanScore(times))}</span>
                                    <label>${category}</label>
                                </span>`;
            }
            summaryHtml += "</div>";
            const summaryElement = document.getElementById("result-summary");
            summaryElement.classList.add("done");
            summaryElement.innerHTML = summaryHtml;
            summaryElement.onclick = displayCategoryScores;
            statusElement.innerHTML = "";
        } else if (!JetStreamParams.dumpJSONResults) {
            console.log("Overall:");
            for (let [category, scores] of categoryScores) {
                console.log(
                    shellFriendlyLabel(`Overall ${category}-Score`),
                    shellFriendlyScore(geomeanScore(scores)));
            }
            for (let [category, times] of categoryTimes) {
                console.log(
                    shellFriendlyLabel(`Overall ${category}-Time`),
                    shellFriendlyDuration(geomeanScore(times)));
            }
            console.log("");
            console.log(shellFriendlyLabel("Overall Score"), shellFriendlyScore(overallScore));
            console.log(shellFriendlyLabel("Overall Wall-Time"), shellFriendlyDuration(totalTime));
            console.log("");
        }

        this.reportScoreToRunBenchmarkRunner();
        this.dumpJSONResultsIfNeeded();
        this.isDone = true;

        if (isInBrowser) {
            globalThis.dispatchEvent(new CustomEvent("JetStreamDone", {
                detail: this.resultsObject()
            }));
        }
    }

    prepareBrowserUI() {
        let text = "";
        for (const benchmark of this.benchmarks)
            text += benchmark.renderHTML();

        const resultsTable = document.getElementById("results");
        resultsTable.innerHTML = text;

        document.getElementById("magic").textContent = "";
        document.addEventListener('keypress', (e) => {
            if (e.key === "Enter")
                JetStream.start();
        });
    }

    reportError(benchmark, error) {
        this.pushError(benchmark.name, error);

        if (!isInBrowser)
            return;

        for (const id of benchmark.allScoreIdentifiers())
            document.getElementById(id).innerHTML = "error";
        for (const id of benchmark.allTimeIdentifiers())
            document.getElementById(id).innerHTML = "error";
        const benchmarkResultsUI = document.getElementById(`benchmark-${benchmark.name}`);
        benchmarkResultsUI.classList.remove("benchmark-running");
        benchmarkResultsUI.classList.add("benchmark-error");
    }

    pushError(name, error) {
        this.errors.push({
            benchmark: name,
            error: error.toString(),
            stack: error.stack
        });
    }

    async initialize() {
        if (isInBrowser)
            window.addEventListener("error", (e) => this.pushError("driver startup", e.error));
        await this.prefetchResources();
        this.benchmarks.sort((a, b) => a.name.toLowerCase() < b.name.toLowerCase() ? 1 : -1);
        if (isInBrowser)
            this.prepareBrowserUI();
        this.isReady = true;
        if (isInBrowser) {
            globalThis.dispatchEvent(new Event("JetStreamReady"));
            if (typeof(JetStreamParams.startDelay) !== "undefined") {
                setTimeout(() => this.start(), JetStreamParams.startDelay);
            }
        }
    }

    async prefetchResources() {
        if (!isInBrowser) {
            if (JetStreamParams.prefetchResources) {
                await zlib.initialize();
            }
            for (const benchmark of this.benchmarks)
                benchmark.prefetchResourcesForShell();
            return;
        }

        // TODO: Cleanup the browser path of the preloading below and in
        // `prefetchResourcesForBrowser` / `retryPrefetchResourcesForBrowser`.
        const counter = JetStream.counter;
        const promises = [];
        for (const benchmark of this.benchmarks)
            promises.push(benchmark.prefetchResourcesForBrowser(counter));
        await Promise.all(promises);

        if (counter.failedPreloadResources || counter.loadedResources != counter.totalResources) {
            for (const benchmark of this.benchmarks) {
                const allFilesLoaded = await benchmark.retryPrefetchResourcesForBrowser(counter);
                if (allFilesLoaded)
                    break;
            }

            if (counter.failedPreloadResources || counter.loadedResources != counter.totalResources) {
                // If we've failed to prefetch resources even after a sequential 1 by 1 retry,
                // then fail out early rather than letting subtests fail with a hang.
                globalThis.allIsGood = false;
                throw new Error("Fetch failed");
            }
        }

        JetStream.loadCache = { }; // Done preloading all the files.

        const statusElement = document.getElementById("status");
        statusElement.classList.remove('loading');
        statusElement.innerHTML = `<a href="javascript:JetStream.start()" class="button">Start Test</a>`;
        statusElement.onclick = () => {
            statusElement.onclick = null;
            JetStream.start();
            return false;
        }
    }

    updateCounterUI() {
        const counter = JetStream.counter;
        const statusElement = document.getElementById("status-text");
        statusElement.innerText = `Loading ${counter.loadedResources} of ${counter.totalResources} ...`;

        const percent = (counter.loadedResources / counter.totalResources) * 100;
        const progressBar = document.getElementById("status-progress-bar");
        progressBar.style.width = `${percent}%`;
    }

    resultsObject(format = "run-benchmark") {
        switch(format) {
            case "run-benchmark":
                return this.runBenchmarkResultsObject();
            case "simple":
                return this.simpleResultsObject();
            default:
                throw Error(`Unknown result format '${format}'`);
        }
    }

    runBenchmarkResultsObject()
    {
        let results = {};
        for (const benchmark of this.benchmarks) {
            const subResults = {}
            const subScores = benchmark.subScores();
            for (const name in subScores) {
                subResults[name] = {"metrics": {"Time": {"current": [toTimeValue(subScores[name])]}}};
            }
            results[benchmark.name] = {
                "metrics" : {
                    "Score" : {"current" : [benchmark.score]},
                    "Time": ["Geometric"],
                },
                "tests": subResults,
            };
        }

        results = {"JetStream3.0": {"metrics" : {"Score" : ["Geometric"]}, "tests" : results}};
        return results;
    }

    simpleResultsObject() {
        const results = {__proto__: null};
        for (const benchmark of this.benchmarks) {
            if (!benchmark.isDone)
                continue;
            if (!benchmark.isSuccess) {
                results[benchmark.name] = "FAILED";
            } else {
                results[benchmark.name] = {
                    Score: benchmark.score,
                    ...benchmark.subScores(),

                };
            }
        }
        return results;
    }

    resultsJSON(format = "run-benchmark")
    {
        return JSON.stringify(this.resultsObject(format));
    }

    dumpJSONResultsIfNeeded()
    {
        if (JetStreamParams.dumpJSONResults) {
            console.log("\n");
            console.log(this.resultsJSON());
            console.log("\n");
        }
    }

    dumpTestList()
    {
        for (const benchmark of this.benchmarks) {
            console.log(benchmark.name);
        }
    }

    async reportScoreToRunBenchmarkRunner()
    {
        if (!isInBrowser)
            return;

        if (!JetStreamParams.report)
            return;

        const content = this.resultsJSON();
        await fetch("/report", {
            method: "POST",
            headers: {
                "Content-Type": "application/json",
                "Content-Length": content.length,
                "Connection": "close",
            },
            body: content,
        });
    }
};

const BenchmarkState = Object.freeze({
    READY: "READY",
    SETUP: "SETUP",
    RUNNING: "RUNNING",
    FINALIZE: "FINALIZE",
    ERROR: "ERROR",
    DONE: "DONE"
})


class Scripts {
    constructor(preloads) {
        this.scripts = [];

        let preloadsCode = "";
        let resourcesCode = "";
        for (let { name, resource, blobURLOrPath } of preloads) {
            console.assert(name?.length > 0, "Invalid preload name.");
            console.assert(resource?.length > 0, "Invalid preload resource.");
            console.assert(blobURLOrPath?.length > 0, "Invalid preload data.");
            preloadsCode += `${JSON.stringify(name)}: "${blobURLOrPath}",\n`;
            resourcesCode += `${JSON.stringify(resource)}: "${blobURLOrPath}",\n`;
        }
        // Expose a globalThis.JetStream object to the workload. We use
        // a proxy to prevent prototype access and throw on unknown properties.
        this.add(`
            const throwOnAccess = (name) => new Proxy({},  {
                get(target, property, receiver) {
                    throw new Error(name + "." + property + " is not defined.");
                }
            });
            globalThis.JetStream = {
                __proto__: throwOnAccess("JetStream"),
                preload: {
                    __proto__: throwOnAccess("JetStream.preload"),
                    ${preloadsCode}
                },
                resources: {
                    __proto__: throwOnAccess("JetStream.preload"),
                    ${resourcesCode}
                },
            };
            `);
        this.add(`
            performance.mark ??= function(name) { return { name }};
            performance.measure ??= function() {};
            performance.timeOrigin ??= performance.now();
        `);
    }


    run() {
        throw new Error("Subclasses need to implement this");
    }

    add(text) {
        throw new Error("Subclasses need to implement this");
    }

    addWithURL(url) {
        throw new Error("addWithURL not supported");
    }

    addBrowserTest() {
        this.add(`
            globalThis.JetStream.isInBrowser = ${isInBrowser};
            globalThis.JetStream.isD8 = ${isD8};
        `);
    }

    addDeterministicRandom() {
        this.add(`(() => {
            const initialSeed = 49734321;
            let seed = initialSeed;

            Math.random = () => {
                // Robert Jenkins' 32 bit integer hash function.
                seed = ((seed + 0x7ed55d16) + (seed << 12))  & 0xffff_ffff;
                seed = ((seed ^ 0xc761c23c) ^ (seed >>> 19)) & 0xffff_ffff;
                seed = ((seed + 0x165667b1) + (seed << 5))   & 0xffff_ffff;
                seed = ((seed + 0xd3a2646c) ^ (seed << 9))   & 0xffff_ffff;
                seed = ((seed + 0xfd7046c5) + (seed << 3))   & 0xffff_ffff;
                seed = ((seed ^ 0xb55a4f09) ^ (seed >>> 16)) & 0xffff_ffff;
                // Note that Math.random should return a value that is
                // greater than or equal to 0 and less than 1. Here, we
                // cast to uint32 first then divided by 2^32 for double.
                return (seed >>> 0) / 0x1_0000_0000;
            };

            Math.random.__resetSeed = () => {
                seed = initialSeed;
            };
        })();`);
    }
}

class ShellScripts extends Scripts {
    constructor(preloads) {
        super(preloads);
        this.prefetchedResources = Object.create(null);;
    }

    run() {
        let globalObject;
        let realm;
        if (isD8) {
            realm = Realm.createAllowCrossRealmAccess();
            globalObject = Realm.global(realm);
            globalObject.loadString = function(s) {
                return Realm.eval(realm, s);
            };
            globalObject.readFile = read;
        } else if (isSpiderMonkey) {
            globalObject = newGlobal();
            globalObject.loadString = globalObject.evaluate;
            globalObject.readFile = globalObject.readRelativeToScript;
        } else
            globalObject = runString("");

        // Expose console copy in the realm so we don't accidentally modify
        // the original object.
        globalObject.console = Object.assign({}, console);
        globalObject.self = globalObject;
        globalObject.top = {
            currentResolve,
            currentReject
        };

        // Pass the prefetched resources to the benchmark global.
        if (JetStreamParams.prefetchResources) {
            // Pass the 'TextDecoder' polyfill into the benchmark global. Don't
            // use 'TextDecoder' as that will get picked up in the kotlin test
            // without full support.
            globalObject.ShellTextDecoder = TextDecoder;
            // Store shellPrefetchedResources on ShellPrefetchedResources so that
            // getBinary and getString can find them.
            globalObject.ShellPrefetchedResources = this.prefetchedResources;
        } else {
            console.assert(Object.values(this.prefetchedResources).length === 0, "Unexpected prefetched resources");
        }

        globalObject.performance ??= performance;
        globalObject.loadString(this.scripts.join("\n"));

        return isD8 ? realm : globalObject;
    }

    addPrefetchedResources(prefetchedResources) {
        for (let [file, bytes] of Object.entries(prefetchedResources)) {
            this.prefetchedResources[file] = bytes;
        }
    }

    add(text) {
        this.scripts.push(text);
    }

    addWithURL(url) {
        console.assert(false, "Should not reach here in CLI");
    }
}

class BrowserScripts extends Scripts {
    constructor(preloads) {
        super(preloads);
        this.add("window.onerror = top.currentReject;");
    }

    run() {
        const string = this.scripts.join("\n");
        const magic = document.getElementById("magic");
        magic.contentDocument.body.textContent = "";
        magic.contentDocument.body.innerHTML = `<iframe id="magicframe" frameborder="0">`;

        const magicFrame = magic.contentDocument.getElementById("magicframe");
        magicFrame.contentDocument.open();
        magicFrame.contentDocument.write(`<!DOCTYPE html>
            <head>
               <title>benchmark payload</title>
            </head>
            <body>${string}</body>
        </html>`);
        return magicFrame;
    }

    add(text) {
        this.scripts.push(`<script>${text}</script>`);
    }

    addWithURL(url) {
        this.scripts.push(`<script src="${url}"></script>`);
    }
}


class Benchmark {
    constructor({
            name,
            files,
            preload = {},
            tags,
            iterations,
            deterministicRandom = false,
            exposeBrowserTest = false,
            allowUtf16 = false,
            args = {} }) {
        this._state = BenchmarkState.READY;
        this.results = [];

        this.name = name
        this.tags = this._processTags(tags)
        this._arguments = args;

        this.iterations = this._processIterationCount(iterations);
        this._deterministicRandom = deterministicRandom;
        this._exposeBrowserTest = exposeBrowserTest;
        this.allowUtf16 = !!allowUtf16;

        // Resource handling:
        this._scripts = null;
        this._files = files;
        this._preloadEntries = Object.entries(preload);
        this._preloadBlobData = [];
        this._shellPrefetchedResources = null;
    }

    // Use getter so it can be overridden in subclasses (GroupedBenchmark).
    get files() {
        return this._files;
    }
    get preloadEntries() {
        return this._preloadEntries;
    }

    _processTags(rawTags) {
        const tags = new Set(rawTags.map(each => each.toLowerCase()));
        if (tags.size != rawTags.length)
            throw new Error(`${this.name} got duplicate tags: ${rawTags.join()}`);
        tags.add("all");
        if (!tags.has("default"))
            tags.add("disabled");
        return tags;
    }

    _processIterationCount(iterations) {
        if (this.name in JetStreamParams.testIterationCountMap)
            return JetStreamParams.testIterationCountMap[this.name];
        if (JetStreamParams.testIterationCount)
            return JetStreamParams.testIterationCount;
        if (iterations)
            return iterations;
        return defaultIterationCount;
    }

    _processWorstCaseCount(worstCaseCount) {
        if (this.name in JetStreamParams.testWorstCaseCountMap)
            return JetStreamParams.testWorstCaseCountMap[this.name];
        if (JetStreamParams.testWorstCaseCount !== undefined)
            return JetStreamParams.testWorstCaseCount;
        if (worstCaseCount !== undefined)
            return worstCaseCount;
        return defaultWorstCaseCount;
    }

    get isDone() {
        return this._state == BenchmarkState.DONE || this._state == BenchmarkState.ERROR;
    }
    get isSuccess() { return this._state = BenchmarkState.DONE; }

    hasAnyTag(...tags) {
        return tags.some((tag) => this.tags.has(tag.toLowerCase()));
    }

    get benchmarkArguments() {
        return {
            ...this._arguments,
            iterationCount: this.iterations,
        };
    }

    get runnerCode() {
        return `{
            const benchmark = new Benchmark(${JSON.stringify(this.benchmarkArguments)});
            const results = [];
            const benchmarkName = "${this.name}";

            for (let i = 0; i < ${this.iterations}; i++) {
                ${this.preIterationCode}

                const iterationMarkLabel = benchmarkName + "-iteration-" + i;
                const iterationStartMark = performance.mark(iterationMarkLabel);

                const start = performance.now();
                benchmark.runIteration(i);
                const end = performance.now();

                performance.measure(iterationMarkLabel, iterationMarkLabel);

                ${this.postIterationCode}

                results.push(Math.max(1, end - start));
            }
            benchmark.validate?.(${this.iterations});
            top.currentResolve(results);
        };`;
    }

    processResults(results) {
        this.results = Array.from(results);
        return this.results;
    }

    get score() {
        const subScores = Object.values(this.subScores());
        return geomeanScore(subScores);
    }

    get totalTime() {
        const subTimes = Object.values(this.subTimes());
        return sum(subTimes);
    }

    get wallTime() {
        return this.endTime - this.startTime;
    }

    subScores() {
        throw new Error("Subclasses need to implement this");
    }

    subTimes() {
        throw new Error("Subclasses need to implement this");
    }

    allScores() {
        const allScores = this.subScores();
        allScores["Score"] = this.score;
        return allScores;
    }

    allTimes() {
        const allTimes = this.subTimes();
        allTimes["Total"] = this.totalTime;
        allTimes["Wall"] = this.wallTime;
        return allTimes;
    }

    get prerunCode() { return null; }


    get preIterationCode() {
        let code = this.prepareForNextIterationCode ;
        if (this._deterministicRandom)
            code += `Math.random.__resetSeed();`;

        if (JetStreamParams.customPreIterationCode)
            code += JetStreamParams.customPreIterationCode;

        return code;
    }

    get prepareForNextIterationCode() {
        return "benchmark.prepareForNextIteration?.();"
    }

    get postIterationCode() {
        let code = "";

        if (JetStreamParams.customPostIterationCode)
            code += JetStreamParams.customPostIterationCode;

        return code;
    }

    renderHTML() {
        const scoreDescription = Object.keys(this.allScores());
        const timeDescription = Object.keys(this.allTimes());

        const scoreIds = this.allScoreIdentifiers();
        const overallScoreId = scoreIds.pop();
        const timeIds = this.allTimeIdentifiers();
        let text = `<div class="benchmark" id="benchmark-${this.name}">
            <h3 class="benchmark-name">${this.name} <a class="info" href="in-depth.html#${this.name}">i</a></h3>
            <h4 class="score" id="${overallScoreId}">&nbsp;</h4>
            <h4 class="plot" id="plot-${this.name}">&nbsp;</h4>
            <p>`;
        for (let i = 0; i < scoreIds.length; i++) {
            const scoreId = scoreIds[i];
            const label = scoreDescription[i];
            text += `<span class="result"><span id="${scoreId}">&nbsp;</span><label>${label}</label></span>`;
        }
        text += "<br/>";
        for (let i = 0; i < timeIds.length; i++) {
            const timeId = timeIds[i];
            const label = timeDescription[i];
            text += `<span class="result detail"><span id="${timeId}">&nbsp;</span><label>${label}</label></span>`;
        }
        text += `</p></div>`;
        return text;
    }

    async run() {
        if (this.isDone)
            throw new Error(`Cannot run Benchmark ${this.name} twice`);
        this._state = BenchmarkState.PREPARE;

        if (JetStreamParams.forceGC) {
            // This will trigger for individual benchmarks in
            // GroupedBenchmarks since they delegate .run() to their inner
            // non-grouped benchmarks.
            globalThis?.gc();
        }

        const scripts = isInBrowser ?
                new BrowserScripts(this._preloadBlobData) :
                new ShellScripts(this._preloadBlobData);

        if (this._deterministicRandom)
            scripts.addDeterministicRandom()
        if (this._exposeBrowserTest)
            scripts.addBrowserTest();

        if (this._shellPrefetchedResources) {
            scripts.addPrefetchedResources(this._shellPrefetchedResources);
        }

        const prerunCode = this.prerunCode;
        if (prerunCode)
            scripts.add(prerunCode);

        if (!isInBrowser) {
            console.assert(this._scripts && this._scripts.length === this.files.length);
            for (const text of this._scripts)
                scripts.add(text);
        } else {
            const cache = browserFileLoader.blobDataCache;
            for (const file of this.files) {
                scripts.addWithURL(cache[file].blobURL);
            }
        }

        const promise = new Promise((resolve, reject) => {
            currentResolve = resolve;
            currentReject = reject;
        });

        scripts.add(this.runnerCode);

        performance.mark(this.name);
        this.startTime = performance.now();

        if (JetStreamParams.RAMification)
            resetMemoryPeak();

        let magicFrame;
        try {
            this._state = BenchmarkState.RUNNING;
            magicFrame = scripts.run();
        } catch(e) {
            this._state = BenchmarkState.ERROR;
            console.log("Error in runCode: ", e);
            console.log(e.stack);
            throw e;
        } finally {
            this._state = BenchmarkState.FINALIZE;
        }
        const results = await promise;

        this.endTime = performance.now();
        performance.measure(this.name, this.name);

        if (JetStreamParams.RAMification) {
            const memoryFootprint = MemoryFootprint();
            this.currentFootprint = memoryFootprint.current;
            this.peakFootprint = memoryFootprint.peak;
        }

        this.processResults(results);
        this._state = BenchmarkState.DONE;

        if (isInBrowser)
            magicFrame.contentDocument.close();
        else if (isD8)
            Realm.dispose(magicFrame);
    }


    updateCounter() {
        const counter = JetStream.counter;
        ++counter.loadedResources;
        JetStream.updateCounterUI();
    }

    prefetchResourcesForBrowser(counter) {
        console.assert(isInBrowser);

        const promises = this.files.map((file) => browserFileLoader.loadBlob("file", null, file).then((blobData) => {
                if (!globalThis.allIsGood)
                    return;
                this.updateCounter();
            }).catch((error) => {
                // We'll try again later in retryPrefetchResourceForBrowser(). Don't throw an error.
            }));

        for (const [name, resource] of this.preloadEntries) {
            promises.push(browserFileLoader.loadBlob("preload", name, resource).then((blobData) => {
                if (!globalThis.allIsGood)
                    return;
                this._preloadBlobData.push({ name: blobData.prop, resource: blobData.resource, blobURLOrPath: blobData.blobURL });
                this.updateCounter();
            }).catch((error) => {
                // We'll try again later in retryPrefetchResourceForBrowser(). Don't throw an error.
                if (!this.failedPreloads)
                    this.failedPreloads = { };
                this.failedPreloads[name] = true;
                counter.failedPreloadResources++;
            }));
        }

        JetStream.counter.totalResources += promises.length;
        return Promise.all(promises);
    }

    async retryPrefetchResourcesForBrowser(counter) {
        // FIXME: Move to BrowserFileLoader.
        console.assert(isInBrowser);

        for (const resource of this.files) {
            const allDone = await browserFileLoader.retryPrefetchResource("file", null, resource);

            if (allDone)
                return true; // All resources loaded, nothing more to do.
        }

        for (const [name, resource] of this.preloadEntries) {
            const allDone = await browserFileLoader.retryPrefetchResource("preload", name, resource);
            if (allDone)
                return true; // All resources loaded, nothing more to do.
        }
        return !counter.failedPreloadResources && counter.loadedResources == counter.totalResources;
    }

    prefetchResourcesForShell() {
        // FIXME: move to ShellFileLoader.
        console.assert(!isInBrowser);

        console.assert(this._scripts === null, "This initialization should be called only once.");
        this._scripts = this.files.map(file => shellFileLoader.load(file));

        console.assert(this._preloadBlobData.length === 0, "This initialization should be called only once.");
        this._shellPrefetchedResources = Object.create(null);
        for (let [name, resource] of this.preloadEntries) {
            const compressed = isCompressed(resource);
            if (compressed && !JetStreamParams.prefetchResources) {
                resource = uncompressedName(resource);
            }

            if (JetStreamParams.prefetchResources) {
                let bytes = new Int8Array(read(resource, "binary"));
                if (compressed) {
                    bytes = zlib.decompress(bytes);
                }
                this._shellPrefetchedResources[resource] = bytes;
            }

            this._preloadBlobData.push({ name, resource, blobURLOrPath: resource });
        }
    }

    allScoreIdentifiers() {
        const ids = Object.keys(this.allScores()).map(name => this.scoreIdentifier(name));
        return ids;
    }

    scoreIdentifier(scoreName) {
        return `results-cell-${this.name}-${scoreName}`;
    }

    allTimeIdentifiers() {
        const ids = Object.keys(this.allTimes()).map(name => this.timeIdentifier(name));
        return ids;
    }

    timeIdentifier(scoreName) {
        return `results-cell-${this.name}-${scoreName}-time`;
    }

    updateUIBeforeRun() {
        if (!JetStreamParams.dumpJSONResults)
            this.updateConsoleBeforeRun();
        if (isInBrowser)
            this.updateUIBeforeRunInBrowser();
    }

    updateConsoleBeforeRun() {
        console.log(`Running ${this.name}:`);
    }

    updateUIBeforeRunInBrowser() {
        const resultsBenchmarkUI = document.getElementById(`benchmark-${this.name}`);
        resultsBenchmarkUI.classList.add("benchmark-running");
        resultsBenchmarkUI.scrollIntoView({ block: "nearest" });

        for (const id of this.allScoreIdentifiers())
            document.getElementById(id).innerHTML = "...";
        for (const id of this.allTimeIdentifiers())
            document.getElementById(id).innerHTML = "...";
    }

    updateUIAfterRun() {
        if (isInBrowser)
            this.updateUIAfterRunInBrowser();
        if (JetStreamParams.dumpJSONResults)
            return;
        this.updateConsoleAfterRun();
    }

    updateUIAfterRunInBrowser() {
        const benchmarkResultsUI = document.getElementById(`benchmark-${this.name}`);
        benchmarkResultsUI.classList.remove("benchmark-running");
        benchmarkResultsUI.classList.add("benchmark-done");

        for (const [name, value] of Object.entries(this.allScores()))
            document.getElementById(this.scoreIdentifier(name)).innerHTML = uiFriendlyScore(value);
        for (const [name, value] of Object.entries(this.allTimes()))
            document.getElementById(this.timeIdentifier(name)).innerHTML = uiFriendlyDuration(value);

        this.renderScatterPlot();
    }

    updateConsoleAfterRun() {
        for (let [name, value] of Object.entries(this.allScores())) {
            if (!name.endsWith("Score"))
                name = `${name}-Score`;

            this.logMetric(name, shellFriendlyScore(value));
        }
        for (let [name, value] of Object.entries(this.allTimes())) {
            this.logMetric(`${name}-Time`, shellFriendlyDuration(value));
        }
        if (JetStreamParams.RAMification) {
            this.logMetric("Current Footprint", uiFriendlyNumber(this.currentFootprint));
            this.logMetric("Peak Footprint", uiFriendlyNumber(this.peakFootprint));
        }
        console.log("");
    }

    logMetric(name, value) {
        console.log(
            shellFriendlyLabel(`${this.name} ${name}`),
            value);
    }

    renderScatterPlot() {
        const plotContainer = document.getElementById(`plot-${this.name}`);
        if (!plotContainer || !this.results || this.results.length === 0)
            return;

        const scores = this.results.map(time => toScore(time));
        const scoreElement = document.getElementById(this.scoreIdentifier("Score"));
        const width = scoreElement.offsetWidth;
        const height = scoreElement.offsetHeight;

        const padding = 5;
        const maxResult = Math.max(...scores);
        const minResult = Math.min(...scores);

        const xRatio = (width - 2 * padding) / (scores.length - 1 || 1);
        const yRatio = (height - 2 * padding) / (maxResult - minResult || 1);
        const radius = Math.max(1.5, Math.min(2.5, 10 - (this.iterations / 10)));

        let circlesSVG = "";
        for (let i = 0; i < scores.length; i++) {
            const result = scores[i];
            const cx = padding + i * xRatio;
            const cy = height - padding - (result - minResult) * yRatio;
            const title = `Iteration ${i + 1}: ${uiFriendlyScore(result)} (${uiFriendlyDuration(this.results[i])})`;
            circlesSVG += `<circle cx="${cx}" cy="${cy}" r="${radius}"><title>${title}</title></circle>`;
        }
        plotContainer.innerHTML = `<svg width="${width}px" height="${height}px">${circlesSVG}</svg>`;
    }
};

class GroupedBenchmark extends Benchmark {
    constructor(plan, benchmarks) {
        super(plan);
        console.assert(benchmarks.length);
        for (const benchmark of benchmarks) {
            // FIXME: Tags don't work for grouped tests anyway but if they did then this would be weird and probably wrong.
            console.assert(!benchmark.hasAnyTag("Default"), `Grouped benchmark sub-benchmarks shouldn't have the "Default" tag`, benchmark.tags);
        }
        benchmarks.sort((a, b) => a.name.toLowerCase() < b.name.toLowerCase() ? 1 : -1);
        this.benchmarks = benchmarks;
    }

    async prefetchResourcesForBrowser(counter) {
        for (const benchmark of this.benchmarks)
            await benchmark.prefetchResourcesForBrowser(counter);
    }

    async retryPrefetchResourcesForBrowser(counter) {
        for (const benchmark of this.benchmarks)
            await benchmark.retryPrefetchResourcesForBrowser(counter);
    }

    prefetchResourcesForShell() {
        for (const benchmark of this.benchmarks)
            benchmark.prefetchResourcesForShell();
    }

    renderHTML() {
        let text = super.renderHTML();
        if (JetStreamParams.groupDetails) {
            for (const benchmark of this.benchmarks)
                text += benchmark.renderHTML();
        }
        return text;
    }

    updateConsoleBeforeRun() {
        if (!JetStreamParams.groupDetails)
            super.updateConsoleBeforeRun();
    }

    updateConsoleAfterRun(scoreEntries) {
        if (JetStreamParams.groupDetails)
            super.updateConsoleBeforeRun();
        super.updateConsoleAfterRun(scoreEntries);
    }

    get files() {
        return this.benchmarks.flatMap(benchmark => benchmark.files)
    }

    get preloadEntries() {
        return this.benchmarks.flatMap(benchmark => benchmark.preloadEntries)
    }

    async run() {
        this._state = BenchmarkState.PREPARE;
        performance.mark(this.name);
        this.startTime = performance.now();

        let benchmark;
        try {
            this._state = BenchmarkState.RUNNING;
            for (benchmark of this.benchmarks) {
                if (JetStreamParams.groupDetails)
                    benchmark.updateUIBeforeRun();
                await benchmark.run();
                if (JetStreamParams.groupDetails)
                    benchmark.updateUIAfterRun();
            }
        } catch (e) {
            this._state = BenchmarkState.ERROR;
            console.log(`Error in runCode of grouped benchmark ${benchmark.name}: `, e);
            console.log(e.stack);
            throw e;
        } finally {
            this._state = BenchmarkState.FINALIZE;
        }

        this.endTime = performance.now();
        performance.measure(this.name, this.name);

        this.processResults();
        this._state = BenchmarkState.DONE;
    }

    processResults() {
        this.results = [];
        for (const benchmark of this.benchmarks)
            this.results = this.results.concat(benchmark.results);
    }

    subScores() {
        const results = {};

        for (const benchmark of this.benchmarks) {
            let scores = benchmark.subScores();
            for (let subScore in scores) {
                results[subScore] ??= [];
                results[subScore].push(scores[subScore]);
            }
        }

        for (let subScore in results)
            results[subScore] = geomeanScore(results[subScore]);
        return results;
    }

    subTimes() {
        const results = {};

        for (const benchmark of this.benchmarks) {
            let times = benchmark.subTimes();
            for (let subTime in times) {
                results[subTime] ??= [];
                results[subTime].push(times[subTime]);
            }
        }

        for (let subTimes in results)
            results[subTimes] = sum(results[subTimes]);
        return results;
    }
};

class DefaultBenchmark extends Benchmark {
    constructor({worstCaseCount, ...args}) {
        super(args);

        this.worstCaseCount = this._processWorstCaseCount(worstCaseCount);
        this.firstIterationTime = null;
        this.firstIterationScore = null;
        this.worstTime = null;
        this.worstScore = null;
        this.averageTime = null;
        this.averageScore = null;
        if (this.worstCaseCount)
            console.assert(this.iterations > this.worstCaseCount);
        console.assert(this.worstCaseCount >= 0);
    }

    processResults(results) {
        results = super.processResults(results)

        this.firstIterationTime = results[0];
        this.firstIterationScore = toScore(results[0]);

        results = results.slice(1);
        results.sort((a, b) => a < b ? 1 : -1);
        for (let i = 0; i + 1 < results.length; ++i)
            console.assert(results[i] >= results[i + 1]);

        if (this.worstCaseCount) {
            const worstCase = [];
            for (let i = 0; i < this.worstCaseCount; ++i)
                worstCase.push(results[i]);
            this.worstTime = mean(worstCase);
            this.worstScore = toScore(this.worstTime);
        }
        this.averageTime = mean(results);
        this.averageScore = toScore(this.averageTime);
    }

    subScores() {
        const scores = { "First": this.firstIterationScore }
        if (this.worstCaseCount)
            scores["Worst"] = this.worstScore;
        if (this.iterations > 1)
            scores["Average"] = this.averageScore;
        return scores;
    }

    subTimes() {
        const times = {
            "First": this.firstIterationTime,
        };
        if (this.worstCaseCount)
            times["Worst"] = this.worstTime;
        if (this.iterations > 1)
            times["Average"] = this.averageTime;
        return times;
    }
}

class AsyncBenchmark extends DefaultBenchmark {
    get prerunCode() {
        let str = "";
        // FIXME: It would be nice if these were available to any benchmark not just async ones but since these functions
        // are async they would only work in a context where the benchmark is async anyway. Long term, we should do away
        // with this class and make all benchmarks async.
        if (isInBrowser) {
            str += `
                JetStream.getBinary = async function(blobURL) {
                    const response = await fetch(blobURL);
                    return new Int8Array(await response.arrayBuffer());
                };

                JetStream.getString = async function(blobURL) {
                    const response = await fetch(blobURL);
                    return response.text();
                };

                JetStream.dynamicImport = async function(blobURL) {
                    return await import(blobURL);
                };
            `;
        } else {
            str += `
                JetStream.getBinary = async function(path) {
                    if ("ShellPrefetchedResources" in globalThis) {
                        return ShellPrefetchedResources[path];
                    }
                    return new Int8Array(read(path, "binary"));
                };

                JetStream.getString = async function(path) {
                    if ("ShellPrefetchedResources" in globalThis) {
                        return new ShellTextDecoder().decode(ShellPrefetchedResources[path]);
                    }
                    return read(path);
                };

                JetStream.dynamicImport = async function(path) {
                    try {
                        // TODO: this skips the prefetched resources, but I'm
                        // not sure of a way around that.
                        return await import(path);
                    } catch (e) {
                        // In shells, relative imports require different paths, so try with and
                        // without the "./" prefix (e.g., JSC requires it).
                        return await import(path.slice("./".length))
                    }
                };
            `;
        }
        return str;
    }

    get prepareForNextIterationCode() {
        return "await benchmark.prepareForNextIteration?.();"
    }

    get runnerCode() {
        return `
        async function doRun() {
            const benchmark = new Benchmark(${JSON.stringify(this.benchmarkArguments)});
            await benchmark.init?.();
            const results = [];
            const benchmarkName = "${this.name}";

            for (let i = 0; i < ${this.iterations}; i++) {
                ${this.preIterationCode}

                const iterationMarkLabel = benchmarkName + "-iteration-" + i;
                const iterationStartMark = performance.mark(iterationMarkLabel);

                const start = performance.now();
                await benchmark.runIteration(i);
                const end = performance.now();

                performance.measure(iterationMarkLabel, iterationMarkLabel);

                ${this.postIterationCode}

                results.push(Math.max(1, end - start));
            }
            benchmark.validate?.(${this.iterations});
            top.currentResolve(results);
        };
        doRun().catch((error) => { top.currentReject(error); });`
    }
};

// Meant for wasm benchmarks that are directly compiled with an emcc build script. It might not work for benchmarks built as
// part of a larger project's build system or a wasm benchmark compiled from a language that doesn't compile with emcc.
class WasmEMCCBenchmark extends AsyncBenchmark {
    get prerunCode() {
        let str = `
            let verbose = false;

            let globalObject = this;

            abort = quit = function() {
                if (verbose)
                    console.log('Intercepted quit/abort');
            };

            const oldPrint = globalObject.print;
            globalObject.print = globalObject.printErr = (...args) => {
                if (verbose)
                    console.log('Intercepted print: ', ...args);
            };

            let Module = {
                preRun: [],
                postRun: [],
                noInitialRun: true,
                print: print,
                printErr: printErr
            };

            globalObject.Module = Module;
            ${super.prerunCode};
        `;

        return str;
    }
};

class WSLBenchmark extends Benchmark {
    constructor(plan) {
        super(plan);

        this.stdlibTime = null;
        this.stdlibScore = null;
        this.mainRunTime = null;
        this.mainRunScore = null;
    }

    processResults(results) {
        results = super.processResults(results);
        this.stdlibTime = results[0];
        this.stdlibScore = toScore(results[0]);
        this.mainRunTime = results[1];
        this.mainRunScore = toScore(results[1]);
    }

    get runnerCode() {
        return `{
            const benchmark = new Benchmark(${JSON.stringify(this.benchmarkArguments)});
            const benchmarkName = "${this.name}";

            const results = [];
            {
                const markLabel = benchmarkName + "-stdlib";
                const startMark = performance.mark(markLabel);

                const start = performance.now();
                benchmark.buildStdlib();
                results.push(performance.now() - start);

                performance.measure(markLabel, markLabel);
            }

            {
                const markLabel = benchmarkName + "-mainRun";
                const startMark = performance.mark(markLabel);

                const start = performance.now();
                benchmark.run();
                results.push(performance.now() - start);

                performance.measure(markLabel, markLabel);
            }
            top.currentResolve(results);
        }`;
    }

    subTimes() {
        return {
            "Stdlib": this.stdlibTime,
            "MainRun": this.mainRunTime,
        };
    }

    subScores() {
        return {
            "Stdlib": this.stdlibScore,
            "MainRun": this.mainRunScore,
        };
    }
};

class AsyncWasmLegacyBenchmark extends Benchmark {
    constructor(plan) {
        super(plan);
        this.startupTime = null;
        this.startupScore = null;
        this.runTime = null;
        this.runScore = null;
    }

    processResults(results) {
        results = super.processResults(results);
        this.startupTime = results[0];
        this.startupScore= toScore(results[0]);
        this.runTime = results[1];
        this.runScore = toScore(results[1]);
    }

    get prerunCode() {
        const str = `
            let verbose = false;

            let compileTime = null;
            let runTime = null;

            let globalObject = this;

            globalObject.benchmarkTime = performance.now.bind(performance);

            globalObject.reportCompileTime = (t) => {
                if (compileTime !== null)
                    throw new Error("called report compile time twice");
                compileTime = t;
            };

            globalObject.reportRunTime = (t) => {
                if (runTime !== null)
                    throw new Error("called report run time twice")
                runTime = t;
                top.currentResolve([compileTime, runTime]);
            };

            abort = quit = function() {
                if (verbose)
                    console.log('Intercepted quit/abort');
            };

            const oldConsoleLog = globalObject.console.log;
            globalObject.print = globalObject.printErr = (...args) => {
                if (verbose)
                    oldConsoleLog('Intercepted print: ', ...args);
            };

            let Module = {
                preRun: [],
                postRun: [],
                print: globalObject.print,
                printErr: globalObject.print
            };
            globalObject.Module = Module;
        `;
        return str;
    }

    get runnerCode() {
        let str = `JetStream.loadBlob = function(key, path, andThen) {`;

        if (isInBrowser) {
            str += `
                const xhr = new XMLHttpRequest();
                xhr.open('GET', path, true);
                xhr.responseType = 'arraybuffer';
                xhr.onload = function() {
                    Module[key] = new Int8Array(xhr.response);
                    andThen();
                };
                xhr.send(null);
            `;
        } else {
            str += `
            if (ShellPrefetchedResources) {
                Module[key] = ShellPrefetchedResources[path];
            } else {
                Module[key] = new Int8Array(read(path, "binary"));
            }
            if (andThen == doRun) {
                globalObject.read = (...args) => {
                    console.log("should not be inside read: ", ...args);
                    throw new Error;
                };
            };

            Promise.resolve(42).then(() => {
                try {
                    andThen();
                } catch(e) {
                    console.log("error running wasm:", e);
                    console.log(e.stack);
                    throw e;
                }
            });
            `;
        }

        str += "};\n";
        let preloadCount = 0;
        for (const [name, resource] of this.preloadEntries) {
            preloadCount++;
            str += `JetStream.loadBlob(${JSON.stringify(name)}, "${resource}", () => {\n`;
        }
        str += `doRun().catch((e) => {
            console.log("error running wasm:", e);
            console.log(e.stack)
            throw e;
        });`;
        for (let i = 0; i < preloadCount; ++i) {
            str += `})`;
        }
        str += `;`;

        return str;
    }

    subScores() {
        return {
            "Startup": this.startupScore,
            "Runtime": this.runScore,
        };
    }

    subTimes() {
        return {
            "Startup": this.startupTime,
            "Runtime": this.runTime,
        };
    }
};

function dotnetPreloads(type)
{
    return {
        dotnetUrl: `./wasm/dotnet/build-${type}/wwwroot/_framework/dotnet.js`,
        dotnetNativeUrl: `./wasm/dotnet/build-${type}/wwwroot/_framework/dotnet.native.js`,
        dotnetRuntimeUrl: `./wasm/dotnet/build-${type}/wwwroot/_framework/dotnet.runtime.js`,
        wasmBinaryUrl: `./wasm/dotnet/build-${type}/wwwroot/_framework/dotnet.native.wasm`,
        icuCustomUrl: `./wasm/dotnet/build-${type}/wwwroot/_framework/icudt_CJK.dat`,
        dllCollectionsConcurrentUrl: `./wasm/dotnet/build-${type}/wwwroot/_framework/System.Collections.Concurrent.wasm`,
        dllCollectionsUrl: `./wasm/dotnet/build-${type}/wwwroot/_framework/System.Collections.wasm`,
        dllComponentModelPrimitivesUrl: `./wasm/dotnet/build-${type}/wwwroot/_framework/System.ComponentModel.Primitives.wasm`,
        dllComponentModelTypeConverterUrl: `./wasm/dotnet/build-${type}/wwwroot/_framework/System.ComponentModel.TypeConverter.wasm`,
        dllDrawingPrimitivesUrl: `./wasm/dotnet/build-${type}/wwwroot/_framework/System.Drawing.Primitives.wasm`,
        dllDrawingUrl: `./wasm/dotnet/build-${type}/wwwroot/_framework/System.Drawing.wasm`,
        dllIOPipelinesUrl: `./wasm/dotnet/build-${type}/wwwroot/_framework/System.IO.Pipelines.wasm`,
        dllLinqUrl: `./wasm/dotnet/build-${type}/wwwroot/_framework/System.Linq.wasm`,
        dllMemoryUrl: `./wasm/dotnet/build-${type}/wwwroot/_framework/System.Memory.wasm`,
        dllObjectModelUrl: `./wasm/dotnet/build-${type}/wwwroot/_framework/System.ObjectModel.wasm`,
        dllPrivateCorelibUrl: `./wasm/dotnet/build-${type}/wwwroot/_framework/System.Private.CoreLib.wasm`,
        dllRuntimeInteropServicesJavaScriptUrl: `./wasm/dotnet/build-${type}/wwwroot/_framework/System.Runtime.InteropServices.JavaScript.wasm`,
        dllTextEncodingsWebUrl: `./wasm/dotnet/build-${type}/wwwroot/_framework/System.Text.Encodings.Web.wasm`,
        dllTextJsonUrl: `./wasm/dotnet/build-${type}/wwwroot/_framework/System.Text.Json.wasm`,
        dllAppUrl: `./wasm/dotnet/build-${type}/wwwroot/_framework/dotnet.wasm`,
    }
}

let BENCHMARKS = [
    // ARES
    new DefaultBenchmark({
        name: "Air",
        files: [
            "./ARES-6/Air/symbols.js",
            "./ARES-6/Air/tmp_base.js",
            "./ARES-6/Air/arg.js",
            "./ARES-6/Air/basic_block.js",
            "./ARES-6/Air/code.js",
            "./ARES-6/Air/frequented_block.js",
            "./ARES-6/Air/inst.js",
            "./ARES-6/Air/opcode.js",
            "./ARES-6/Air/reg.js",
            "./ARES-6/Air/stack_slot.js",
            "./ARES-6/Air/tmp.js",
            "./ARES-6/Air/util.js",
            "./ARES-6/Air/custom.js",
            "./ARES-6/Air/liveness.js",
            "./ARES-6/Air/insertion_set.js",
            "./ARES-6/Air/allocate_stack.js",
            "./ARES-6/Air/payload-gbemu-executeIteration.js",
            "./ARES-6/Air/payload-imaging-gaussian-blur-gaussianBlur.js",
            "./ARES-6/Air/payload-airjs-ACLj8C.js",
            "./ARES-6/Air/payload-typescript-scanIdentifier.js",
            "./ARES-6/Air/benchmark.js",
        ],
        tags: ["default", "js", "ARES"],
    }),
    new DefaultBenchmark({
        name: "Basic",
        files: [
            "./ARES-6/Basic/ast.js",
            "./ARES-6/Basic/basic.js",
            "./ARES-6/Basic/caseless_map.js",
            "./ARES-6/Basic/lexer.js",
            "./ARES-6/Basic/number.js",
            "./ARES-6/Basic/parser.js",
            "./ARES-6/Basic/random.js",
            "./ARES-6/Basic/state.js",
            "./ARES-6/Basic/benchmark.js",
        ],
        tags: ["default", "js",  "ARES"],
    }),
    new DefaultBenchmark({
        name: "ML",
        files: [
            "./ARES-6/ml/index.js",
            "./ARES-6/ml/benchmark.js",
        ],
        iterations: 60,
        tags: ["default", "js",  "ARES"],
    }),
    new AsyncBenchmark({
        name: "Babylon",
        files: [
            "./ARES-6/Babylon/index.js",
            "./ARES-6/Babylon/benchmark.js",
        ],
        preload: {
            airBlob: "./ARES-6/Babylon/air-blob.js",
            basicBlob: "./ARES-6/Babylon/basic-blob.js",
            inspectorBlob: "./ARES-6/Babylon/inspector-blob.js",
            babylonBlob: "./ARES-6/Babylon/babylon-blob.js",
        },
        tags: ["default", "js",  "ARES"],
        allowUtf16: true,
    }),
    // CDJS
    new DefaultBenchmark({
        name: "cdjs",
        files: [
            "./cdjs/constants.js",
            "./cdjs/util.js",
            "./cdjs/red_black_tree.js",
            "./cdjs/call_sign.js",
            "./cdjs/vector_2d.js",
            "./cdjs/vector_3d.js",
            "./cdjs/motion.js",
            "./cdjs/reduce_collision_set.js",
            "./cdjs/simulator.js",
            "./cdjs/collision.js",
            "./cdjs/collision_detector.js",
            "./cdjs/benchmark.js",
        ],
        iterations: 60,
        worstCaseCount: 3,
        tags: ["default", "js",  ],
    }),
    // CodeLoad
    new AsyncBenchmark({
        name: "first-inspector-code-load",
        files: [
            "./code-load/code-first-load.js",
        ],
        preload: {
            inspectorPayloadBlob: "./code-load/inspector-payload-minified.js",
        },
        tags: ["default", "js", "inspector", "codeload"],
    }),
    new AsyncBenchmark({
        name: "multi-inspector-code-load",
        files: [
            "./code-load/code-multi-load.js",
        ],
        preload: {
            inspectorPayloadBlob: "./code-load/inspector-payload-minified.js",
        },
        tags: ["default", "js", "inspector", "codeload"],
    }),
    // Octane
    new DefaultBenchmark({
        name: "Box2D",
        files: [
            "./Octane/box2d.js",
        ],
        deterministicRandom: true,
        tags: ["default", "js",  "Octane"],
    }),
    new DefaultBenchmark({
        name: "octane-code-load",
        files: [
            "./Octane/code-first-load.js",
        ],
        deterministicRandom: true,
        tags: ["default", "js", "codeload", "Octane"],
    }),
    new DefaultBenchmark({
        name: "crypto",
        files: [
            "./Octane/crypto.js",
        ],
        deterministicRandom: true,
        tags: ["default", "js",  "Octane"],
    }),
    new DefaultBenchmark({
        name: "delta-blue",
        files: [
            "./Octane/deltablue.js"
        ],
        deterministicRandom: true,
        tags: ["default", "js",  "Octane"],
    }),
    new DefaultBenchmark({
        name: "earley-boyer",
        files: [
            "./Octane/earley-boyer.js"
        ],
        deterministicRandom: true,
        tags: ["default", "js",  "Octane"],
    }),
    new DefaultBenchmark({
        name: "gbemu",
        files: [
            "./Octane/gbemu-part1.js",
            "./Octane/gbemu-part2.js",
        ],
        deterministicRandom: true,
        tags: ["default", "js",  "Octane"],
    }),
    new DefaultBenchmark({
        name: "mandreel",
        files: [
            "./Octane/mandreel.js"
        ],
        iterations: 80,
        deterministicRandom: true,
        tags: ["default", "js",  "Octane"],
    }),
    new DefaultBenchmark({
        name: "navier-stokes",
        files: [
            "./Octane/navier-stokes.js",
        ],
        deterministicRandom: true,
        tags: ["default",  "js", "Octane"],
    }),
    new DefaultBenchmark({
        name: "pdfjs",
        files: [
            "./Octane/pdfjs.js",
        ],
        deterministicRandom: true,
        tags: ["default", "js",  "Octane"],
    }),
    new DefaultBenchmark({
        name: "raytrace",
        files: [
            "./Octane/raytrace.js",
        ],
        tags: ["default", "js",  "Octane"],
    }),
    new DefaultBenchmark({
        name: "regexp-octane",
        files: [
            "./Octane/regexp.js",
        ],
        deterministicRandom: true,
        tags: ["default", "js", "regexp", "Octane"],
    }),
    new DefaultBenchmark({
        name: "richards",
        files: [
            "./Octane/richards.js",
        ],
        deterministicRandom: true,
        tags: ["default", "js",  "Octane"],
    }),
    new DefaultBenchmark({
        name: "splay",
        files: [
            "./Octane/splay.js",
        ],
        deterministicRandom: true,
        tags: ["default", "js",  "Octane"],
    }),
    new DefaultBenchmark({
        name: "typescript-octane",
        files: [
            "./Octane/typescript-compiler.js",
            "./Octane/typescript-input.js",
            "./Octane/typescript.js",
        ],
        iterations: 15,
        worstCaseCount: 2,
        deterministicRandom: true,
        tags: ["Octane", "js",  "typescript"],
    }),
    // RexBench
    new DefaultBenchmark({
        name: "FlightPlanner",
        files: [
            "./RexBench/FlightPlanner/airways.js",
            "./RexBench/FlightPlanner/waypoints.js.z",
            "./RexBench/FlightPlanner/flight_planner.js",
            "./RexBench/FlightPlanner/expectations.js",
            "./RexBench/FlightPlanner/benchmark.js",
        ],
        tags: ["default", "js",  "RexBench"],
    }),
    new DefaultBenchmark({
        name: "OfflineAssembler",
        files: [
            "./RexBench/OfflineAssembler/registers.js",
            "./RexBench/OfflineAssembler/instructions.js",
            "./RexBench/OfflineAssembler/ast.js",
            "./RexBench/OfflineAssembler/parser.js",
            "./RexBench/OfflineAssembler/file.js",
            "./RexBench/OfflineAssembler/LowLevelInterpreter.js",
            "./RexBench/OfflineAssembler/LowLevelInterpreter32_64.js",
            "./RexBench/OfflineAssembler/LowLevelInterpreter64.js",
            "./RexBench/OfflineAssembler/InitBytecodes.js",
            "./RexBench/OfflineAssembler/expected.js",
            "./RexBench/OfflineAssembler/benchmark.js",
        ],
        iterations: 80,
        tags: ["default", "js",  "RexBench"],
    }),
    new DefaultBenchmark({
        name: "UniPoker",
        files: [
            "./RexBench/UniPoker/poker.js",
            "./RexBench/UniPoker/expected.js",
            "./RexBench/UniPoker/benchmark.js",
        ],
        deterministicRandom: true,
        // FIXME: UniPoker should not access isInBrowser.
        exposeBrowserTest: true,
        tags: ["default", "js",  "RexBench"],
    }),
    new DefaultBenchmark({
        name: "validatorjs",
        files: [
            // Use the unminified version for easier local profiling.
            // "./validatorjs/dist/bundle.es6.js",
            "./validatorjs/dist/bundle.es6.min.js",
            "./validatorjs/benchmark.js",
        ],
        tags: ["default", "js",  "regexp"],
    }),
    // Simple
    new DefaultBenchmark({
        name: "hash-map",
        files: [
            "./simple/hash-map.js",
        ],
        tags: ["default", "js",  "Simple"],
    }),
    new AsyncBenchmark({
        name: "doxbee-promise",
        files: [
            "./simple/doxbee-promise.js",
        ],
        tags: ["default",  "js", "promise", "Simple"],
    }),
    new AsyncBenchmark({
        name: "doxbee-async",
        files: [
            "./simple/doxbee-async.js",
        ],
        tags: ["default", "js", "Simple"],
    }),
    // SeaMonster
    new DefaultBenchmark({
        name: "ai-astar",
        files: [
            "./SeaMonster/ai-astar.js"
        ],
        tags: ["default", "js", "SeaMonster"],
    }),
    new DefaultBenchmark({
        name: "gaussian-blur",
        files: [
            "./SeaMonster/gaussian-blur.js",
        ],
        tags: ["default", "js", "SeaMonster"],
    }),
    new DefaultBenchmark({
        name: "stanford-crypto-aes",
        files: [
            "./SeaMonster/sjlc.js",
            "./SeaMonster/stanford-crypto-aes.js",
        ],
        tags: ["default", "js", "SeaMonster"],
    }),
    new DefaultBenchmark({
        name: "stanford-crypto-pbkdf2",
        files: [
            "./SeaMonster/sjlc.js",
            "./SeaMonster/stanford-crypto-pbkdf2.js"
        ],
        tags: ["default", "js", "SeaMonster"],
    }),
    new DefaultBenchmark({
        name: "stanford-crypto-sha256",
        files: [
            "./SeaMonster/sjlc.js",
            "./SeaMonster/stanford-crypto-sha256.js",
        ],
        tags: ["default", "js", "SeaMonster"],
    }),
    new DefaultBenchmark({
        name: "json-stringify-inspector",
        files: [
            "./SeaMonster/inspector-json-payload.js.z",
            "./SeaMonster/json-stringify-inspector.js",
        ],
        iterations: 20,
        worstCaseCount: 2,
        tags: ["default", "js", "json", "inspector", "SeaMonster"],
    }),
    new DefaultBenchmark({
        name: "json-parse-inspector",
        files: [
            "./SeaMonster/inspector-json-payload.js.z",
            "./SeaMonster/json-parse-inspector.js",
        ],
        iterations: 20,
        worstCaseCount: 2,
        tags: ["default", "js", "json", "inspector", "SeaMonster"],
    }),
    // BigInt
    new AsyncBenchmark({
        name: "bigint-noble-bls12-381",
        files: [
            "./bigint/web-crypto-sham.js",
            "./bigint/noble-bls12-381-bundle.js",
            "./bigint/noble-benchmark.js",
        ],
        iterations: 4,
        worstCaseCount: 1,
        deterministicRandom: true,
        tags: ["js", "bigint", "BigIntNoble"],
    }),
    new AsyncBenchmark({
        name: "bigint-noble-secp256k1",
        files: [
            "./bigint/web-crypto-sham.js",
            "./bigint/noble-secp256k1-bundle.js",
            "./bigint/noble-benchmark.js",
        ],
        deterministicRandom: true,
        tags: ["js", "bigint", "BigIntNoble"],
    }),
    new AsyncBenchmark({
        name: "bigint-noble-ed25519",
        files: [
            "./bigint/web-crypto-sham.js",
            "./bigint/noble-ed25519-bundle.js",
            "./bigint/noble-benchmark.js",
        ],
        iterations: 30,
        deterministicRandom: true,
        tags: ["default", "js", "bigint", "BigIntNoble"],
    }),
    new DefaultBenchmark({
        name: "bigint-paillier",
        files: [
            "./bigint/web-crypto-sham.js",
            "./bigint/paillier-bundle.js",
            "./bigint/paillier-benchmark.js",
        ],
        iterations: 10,
        worstCaseCount: 2,
        deterministicRandom: true,
        tags: ["js", "bigint", "BigIntMisc"],
    }),
    new DefaultBenchmark({
        name: "bigint-bigdenary",
        files: [
            "./bigint/bigdenary-bundle.js",
            "./bigint/bigdenary-benchmark.js",
        ],
        iterations: 160,
        worstCaseCount: 16,
        tags: ["js", "bigint", "BigIntMisc"],
    }),
    // Proxy
    new AsyncBenchmark({
        name: "proxy-mobx",
        files: [
            "./proxy/common.js",
            "./proxy/mobx-bundle.js",
            "./proxy/mobx-benchmark.js",
        ],
        iterations: defaultIterationCount * 3,
        worstCaseCount: defaultWorstCaseCount * 3,
        tags: ["default", "js", "Proxy"],
    }),
    new AsyncBenchmark({
        name: "proxy-vue",
        files: [
            "./proxy/common.js",
            "./proxy/vue-bundle.js",
            "./proxy/vue-benchmark.js",
        ],
        tags: ["default", "js", "Proxy"],
    }),
    new AsyncBenchmark({
        name: "mobx-startup",
        files: [
            "./utils/StartupBenchmark.js",
            "./mobx/benchmark.js",
        ],
        preload: {
            // Debug Sources for nicer profiling.
            // BUNDLE: "./mobx/dist/bundle.es6.js",
            BUNDLE: "./mobx/dist/bundle.es6.min.js",
        },
        tags: ["default", "js", "mobx", "startup", "es6"],
        iterations: 30,
        worstCaseCount: 3,
    }),
    new AsyncBenchmark({
        name: "jsdom-d3-startup",
        files: [
            "./utils/StartupBenchmark.js",
            "./jsdom-d3-startup/benchmark.js",
        ],
        preload: {
            // Unminified sources for profiling.
            // BUNDLE: "./jsdom-d3-startup/dist/bundle.js",
            BUNDLE: "./jsdom-d3-startup/dist/bundle.min.js",
            US_DATA: "./jsdom-d3-startup/data/counties-albers-10m.json",
            AIRPORTS: "./jsdom-d3-startup/data/airports.csv",
        },
        tags: ["default", "js", "d3", "startup", "jsdom"],
        iterations: 15,
        worstCaseCount: 2,
    }),
    new AsyncBenchmark({
        name: "web-ssr",
        files: [
            "./utils/StartupBenchmark.js",
            "./web-ssr/benchmark.js",
        ],
        preload: {
            // Debug Sources for nicer profiling.
            // BUNDLE: "./web-ssr/dist/bundle.js",
            BUNDLE: "./web-ssr/dist/bundle.min.js",
        },
        tags: ["default", "js", "web", "ssr"],
        iterations: 30,
    }),
    // Class fields
    new DefaultBenchmark({
        name: "raytrace-public-class-fields",
        files: [
            "./class-fields/raytrace-public-class-fields.js",
        ],
        tags: ["default", "js", "ClassFields"],
    }),
    new DefaultBenchmark({
        name: "raytrace-private-class-fields",
        files: [
            "./class-fields/raytrace-private-class-fields.js",
        ],
        tags: ["default", "js", "ClassFields"],
    }),
    new AsyncBenchmark({
        name: "typescript-lib",
        files: [
            "./TypeScript/src/mock/sys.js",
            "./TypeScript/dist/bundle.js",
            "./TypeScript/benchmark.js",
        ],
        preload: {
            // Large test project:
            // "tsconfig": "./TypeScript/src/gen/zod-medium/tsconfig.json",
            // "files": "./TypeScript/src/gen/zod-medium/files.json",
            "tsconfig": "./TypeScript/src/gen/immer-tiny/tsconfig.json",
            "files": "./TypeScript/src/gen/immer-tiny/files.json",
        },
        iterations: 1,
        worstCaseCount: 0,
        tags: ["default", "js", "typescript"],
    }),
    // Generators
    new AsyncBenchmark({
        name: "async-fs",
        files: [
            "./generators/async-file-system.js",
        ],
        iterations: 80,
        worstCaseCount: 6,
        deterministicRandom: true,
        tags: ["default", "js", "Generators"],
    }),
    new DefaultBenchmark({
        name: "sync-fs",
        files: [
            "./generators/sync-file-system.js",
        ],
        iterations: 80,
        worstCaseCount: 6,
        deterministicRandom: true,
        tags: ["default", "js", "Generators"],
    }),
    new DefaultBenchmark({
        name: "lazy-collections",
        files: [
            "./generators/lazy-collections.js",
        ],
        tags: ["default", "js", "Generators"],
    }),
    new DefaultBenchmark({
        name: "js-tokens",
        files: [
            "./generators/js-tokens.js",
        ],
        tags: ["default", "js", "Generators"],
    }),
    new DefaultBenchmark({
        name: "threejs",
        files: [
            "./threejs/three.js",
            "./threejs/benchmark.js",
        ],
        deterministicRandom: true,
        tags: ["default", "js"],
    }),
    // Wasm
    new WasmEMCCBenchmark({
        name: "HashSet-wasm",
        files: [
            "./wasm/HashSet/build/HashSet.js",
            "./wasm/HashSet/benchmark.js",
        ],
        preload: {
            wasmBinary: "./wasm/HashSet/build/HashSet.wasm",
        },
        iterations: 50,
        // No longer run by-default: We have more realistic Wasm workloads by
        // now, and it was over-incentivizing inlining.
        tags: ["Wasm"],
    }),
    new WasmEMCCBenchmark({
        name: "quicksort-wasm",
        files: [
            "./wasm/quicksort/build/quicksort.js",
            "./wasm/quicksort/benchmark.js",
        ],
        preload: {
            wasmBinary: "./wasm/quicksort/build/quicksort.wasm",
        },
        iterations: 50,
        // No longer run by-default: We have more realistic Wasm workloads by
        // now, and it was a small microbenchmark.
        tags: ["Wasm"],
    }),
    new WasmEMCCBenchmark({
        name: "gcc-loops-wasm",
        files: [
            "./wasm/gcc-loops/build/gcc-loops.js",
            "./wasm/gcc-loops/benchmark.js",
        ],
        preload: {
            wasmBinary: "./wasm/gcc-loops/build/gcc-loops.wasm",
        },
        iterations: 50,
        // No longer run by-default: We have more realistic Wasm workloads by
        // now, and it was a small microbenchmark.
        tags: ["Wasm"],
    }),
    new WasmEMCCBenchmark({
        name: "tsf-wasm",
        files: [
            "./wasm/TSF/build/tsf.js",
            "./wasm/TSF/benchmark.js",
        ],
        preload: {
            wasmBinary: "./wasm/TSF/build/tsf.wasm",
        },
        iterations: 50,
        tags: ["default", "Wasm"],
    }),
    new WasmEMCCBenchmark({
        name: "richards-wasm",
        files: [
            "./wasm/richards/build/richards.js",
            "./wasm/richards/benchmark.js",
        ],
        preload: {
            wasmBinary: "./wasm/richards/build/richards.wasm",
        },
        iterations: 50,
        tags: ["default", "Wasm"],
    }),
    new WasmEMCCBenchmark({
        name: "sqlite3-wasm",
        files: [
            "./utils/polyfills/fast-text-encoding/1.0.3/text.js",
            "./sqlite3/benchmark.js",
            "./sqlite3/build/jswasm/speedtest1.js",
        ],
        preload: {
            wasmBinary: "./sqlite3/build/jswasm/speedtest1.wasm",
        },
        iterations: 30,
        worstCaseCount: 2,
        tags: ["default", "Wasm"],
    }),
    new WasmEMCCBenchmark({
        name: "Dart-flute-complex-wasm",
        files: [
            "./Dart/benchmark.js",
        ],
        preload: {
            jsModule: "./Dart/build/flute.complex.dart2wasm.mjs",
            wasmBinary: "./Dart/build/flute.complex.dart2wasm.wasm",
        },
        iterations: 15,
        worstCaseCount: 2,
        // Not run by default because the `CupertinoTimePicker` widget is very allocation-heavy,
        // leading to an unrealistic GC-dominated workload. See
        // https://github.com/WebKit/JetStream/pull/97#issuecomment-3139924169
        // The todomvc workload below is less allocation heavy and a replacement for now.
        // TODO: Revisit, once Dart/Flutter worked on this widget or workload.
        tags: ["Wasm"],
    }),
    new WasmEMCCBenchmark({
        name: "Dart-flute-todomvc-wasm",
        files: [
            "./Dart/benchmark.js",
        ],
        preload: {
            jsModule: "./Dart/build/flute.todomvc.dart2wasm.mjs",
            wasmBinary: "./Dart/build/flute.todomvc.dart2wasm.wasm",
        },
        iterations: 30,
        worstCaseCount: 2,
        tags: ["default", "Wasm"],
    }),
    new WasmEMCCBenchmark({
        name: "Kotlin-compose-wasm",
        files: [
            "./Kotlin-compose/benchmark.js",
        ],
        preload: {
            skikoJsModule: "./Kotlin-compose/build/skiko.mjs",
            skikoWasmBinary: "./Kotlin-compose/build/skiko.wasm",
            composeJsModule: "./Kotlin-compose/build/compose-benchmarks-benchmarks.uninstantiated.mjs",
            composeWasmBinary: "./Kotlin-compose/build/compose-benchmarks-benchmarks.wasm",
            inputImageCompose: "./Kotlin-compose/build/compose-multiplatform.png",
            inputImageCat: "./Kotlin-compose/build/example1_cat.jpg",
            inputImageComposeCommunity: "./Kotlin-compose/build/example1_compose-community-primary.png",
            inputFontItalic: "./Kotlin-compose/build/jetbrainsmono_italic.ttf",
            inputFontRegular: "./Kotlin-compose/build/jetbrainsmono_regular.ttf"
        },
        iterations: 5,
        worstCaseCount: 1,
        tags: ["default", "Wasm"],
    }),
    new AsyncBenchmark({
        name: "transformersjs-bert-wasm",
        files: [
            "./utils/polyfills/fast-text-encoding/1.0.3/text.js",
            "./transformersjs/benchmark.js",
            "./transformersjs/task-bert.js",
        ],
        preload: {
            transformersJsModule: "./transformersjs/build/transformers.js",

            onnxJsModule: "./transformersjs/build/onnxruntime-web/ort-wasm-simd-threaded.mjs",
            onnxWasmBinary: "./transformersjs/build/onnxruntime-web/ort-wasm-simd-threaded.wasm",

            modelWeights: "./transformersjs/build/models/Xenova/distilbert-base-uncased-finetuned-sst-2-english/onnx/model_uint8.onnx",
            modelConfig: "./transformersjs/build/models/Xenova/distilbert-base-uncased-finetuned-sst-2-english/config.json",
            modelTokenizer: "./transformersjs/build/models/Xenova/distilbert-base-uncased-finetuned-sst-2-english/tokenizer.json",
            modelTokenizerConfig: "./transformersjs/build/models/Xenova/distilbert-base-uncased-finetuned-sst-2-english/tokenizer_config.json",
        },
        iterations: 30,
        allowUtf16: true,
        tags: ["default", "Wasm", "transformersjs"],
    }),
    new AsyncBenchmark({
        name: "transformersjs-whisper-wasm",
        files: [
            "./utils/polyfills/fast-text-encoding/1.0.3/text.js",
            "./transformersjs/benchmark.js",
            "./transformersjs/task-whisper.js",
        ],
        preload: {
            transformersJsModule: "./transformersjs/build/transformers.js",

            onnxJsModule: "./transformersjs/build/onnxruntime-web/ort-wasm-simd-threaded.mjs",
            onnxWasmBinary: "./transformersjs/build/onnxruntime-web/ort-wasm-simd-threaded.wasm",

            modelEncoderWeights: "./transformersjs/build/models/Xenova/whisper-tiny.en/onnx/encoder_model_quantized.onnx",
            modelDecoderWeights: "./transformersjs/build/models/Xenova/whisper-tiny.en/onnx/decoder_model_merged_quantized.onnx",
            modelConfig: "./transformersjs/build/models/Xenova/whisper-tiny.en/config.json",
            modelTokenizer: "./transformersjs/build/models/Xenova/whisper-tiny.en/tokenizer.json",
            modelTokenizerConfig: "./transformersjs/build/models/Xenova/whisper-tiny.en/tokenizer_config.json",
            modelPreprocessorConfig: "./transformersjs/build/models/Xenova/whisper-tiny.en/preprocessor_config.json",
            modelGenerationConfig: "./transformersjs/build/models/Xenova/whisper-tiny.en/generation_config.json",

            inputFile: "./transformersjs/build/inputs/jfk.raw",
        },
        iterations: 5,
        worstCaseCount: 1,
        allowUtf16: true,
        tags: ["Wasm", "transformersjs"],
    }),
    new AsyncWasmLegacyBenchmark({
        name: "tfjs-wasm",
        files: [
            "./wasm/tfjs-model-helpers.js",
            "./wasm/tfjs-model-mobilenet-v3.js",
            "./wasm/tfjs-model-mobilenet-v1.js",
            "./wasm/tfjs-model-coco-ssd.js",
            "./wasm/tfjs-model-use.js",
            "./wasm/tfjs-model-use-vocab.js",
            "./wasm/tfjs-bundle.js",
            "./wasm/tfjs.js",
            "./wasm/tfjs-benchmark.js",
        ],
        preload: {
            tfjsBackendWasmBlob: "./wasm/tfjs-backend-wasm.wasm",
        },
        deterministicRandom: true,
        exposeBrowserTest: true,
        allowUtf16: true,
        tags: ["Wasm"],
    }),
    new AsyncWasmLegacyBenchmark({
        name: "tfjs-wasm-simd",
        files: [
            "./wasm/tfjs-model-helpers.js",
            "./wasm/tfjs-model-mobilenet-v3.js",
            "./wasm/tfjs-model-mobilenet-v1.js",
            "./wasm/tfjs-model-coco-ssd.js",
            "./wasm/tfjs-model-use.js",
            "./wasm/tfjs-model-use-vocab.js",
            "./wasm/tfjs-bundle.js",
            "./wasm/tfjs.js",
            "./wasm/tfjs-benchmark.js",
        ],
        preload: {
            tfjsBackendWasmSimdBlob: "./wasm/tfjs-backend-wasm-simd.wasm",
        },
        deterministicRandom: true,
        exposeBrowserTest: true,
        allowUtf16: true,
        tags: ["Wasm"],
    }),
    new WasmEMCCBenchmark({
        name: "argon2-wasm",
        files: [
            "./wasm/argon2/build/argon2.js",
            "./wasm/argon2/benchmark.js",
        ],
        preload: {
            wasmBinary: "./wasm/argon2/build/argon2.wasm.z",
        },
        iterations: 30,
        worstCaseCount: 3,
        deterministicRandom: true,
        allowUtf16: true,
        tags: ["default", "Wasm"],
    }),
    new AsyncBenchmark({
        name: "babylonjs-startup-es5",
        files: [
            "./utils/StartupBenchmark.js",
            "./babylonjs/benchmark/startup.js",
        ],
        preload: {
            BUNDLE: "./babylonjs/dist/bundle.es5.min.js",
        },
        args: {
            expectedCacheCommentCount: 23988,
        },
        tags: ["startup",  "js", "class", "es5", "babylonjs"],
        iterations: 10,
    }),
    new AsyncBenchmark({
        name: "babylonjs-startup-es6",
        files: [
            "./utils/StartupBenchmark.js",
            "./babylonjs/benchmark/startup.js",
        ],
        preload: {
            BUNDLE: "./babylonjs/dist/bundle.es6.min.js",
        },
        args: {
            expectedCacheCommentCount: 21222,
        },
        tags: ["Default",  "js", "startup", "class", "es6", "babylonjs"],
        iterations: 10,
    }),
    new AsyncBenchmark({
        name: "babylonjs-scene-es5",
        files: [
            // Use non-minified sources for easier profiling:
            // "./babylonjs/dist/bundle.es5.js",
            "./babylonjs/dist/bundle.es5.min.js",
            "./babylonjs/benchmark/scene.js",
        ],
        preload: {
            PARTICLES_BLOB: "./babylonjs/data/particles.json",
            PIRATE_FORT_BLOB: "./babylonjs/data/pirateFort.glb",
            CANNON_BLOB: "./babylonjs/data/cannon.glb",
        },
        tags: ["scene", "js",  "es5", "babylonjs"],
        iterations: 5,
    }),
    new AsyncBenchmark({
        name: "babylonjs-scene-es6",
        files: [
            // Use non-minified sources for easier profiling:
            // "./babylonjs/dist/bundle.es6.js",
            "./babylonjs/dist/bundle.es6.min.js",
            "./babylonjs/benchmark/scene.js",
        ],
        preload: {
            PARTICLES_BLOB: "./babylonjs/data/particles.json",
            PIRATE_FORT_BLOB: "./babylonjs/data/pirateFort.glb",
            CANNON_BLOB: "./babylonjs/data/cannon.glb",
        },
        tags: ["Default", "js", "scene", "es6", "babylonjs"],
        iterations: 5,
    }),
    // WorkerTests
    new AsyncBenchmark({
        name: "bomb-workers",
        files: [
            "./worker/bomb.js",
        ],
        exposeBrowserTest: true,
        iterations: 80,
        preload: {
            rayTrace3D: "./worker/bomb-subtests/3d-raytrace.js",
            accessNbody: "./worker/bomb-subtests/access-nbody.js",
            morph3D: "./worker/bomb-subtests/3d-morph.js",
            cube3D: "./worker/bomb-subtests/3d-cube.js",
            accessFunnkuch: "./worker/bomb-subtests/access-fannkuch.js",
            accessBinaryTrees: "./worker/bomb-subtests/access-binary-trees.js",
            accessNsieve: "./worker/bomb-subtests/access-nsieve.js",
            bitopsBitwiseAnd: "./worker/bomb-subtests/bitops-bitwise-and.js",
            bitopsNsieveBits: "./worker/bomb-subtests/bitops-nsieve-bits.js",
            controlflowRecursive: "./worker/bomb-subtests/controlflow-recursive.js",
            bitops3BitBitsInByte: "./worker/bomb-subtests/bitops-3bit-bits-in-byte.js",
            botopsBitsInByte: "./worker/bomb-subtests/bitops-bits-in-byte.js",
            cryptoAES: "./worker/bomb-subtests/crypto-aes.js",
            cryptoMD5: "./worker/bomb-subtests/crypto-md5.js",
            cryptoSHA1: "./worker/bomb-subtests/crypto-sha1.js",
            dateFormatTofte: "./worker/bomb-subtests/date-format-tofte.js",
            dateFormatXparb: "./worker/bomb-subtests/date-format-xparb.js",
            mathCordic: "./worker/bomb-subtests/math-cordic.js",
            mathPartialSums: "./worker/bomb-subtests/math-partial-sums.js",
            mathSpectralNorm: "./worker/bomb-subtests/math-spectral-norm.js",
            stringBase64: "./worker/bomb-subtests/string-base64.js",
            stringFasta: "./worker/bomb-subtests/string-fasta.js",
            stringValidateInput: "./worker/bomb-subtests/string-validate-input.js",
            stringTagcloud: "./worker/bomb-subtests/string-tagcloud.js",
            stringUnpackCode: "./worker/bomb-subtests/string-unpack-code.js",
            regexpDNA: "./worker/bomb-subtests/regexp-dna.js",
        },
        tags: ["default", "js", "WorkerTests"],
    }),
    new AsyncBenchmark({
        name: "segmentation",
        files: [
            "./worker/segmentation.js",
        ],
        preload: {
            asyncTaskBlob: "./worker/async-task.js",
        },
        iterations: 36,
        worstCaseCount: 3,
        tags: ["default", "js",  "WorkerTests"],
    }),
    // WSL
    new WSLBenchmark({
        name: "WSL",
        files: [
            "./WSL/Node.js",
            "./WSL/Type.js",
            "./WSL/ReferenceType.js",
            "./WSL/Value.js",
            "./WSL/Expression.js",
            "./WSL/Rewriter.js",
            "./WSL/Visitor.js",
            "./WSL/CreateLiteral.js",
            "./WSL/CreateLiteralType.js",
            "./WSL/PropertyAccessExpression.js",
            "./WSL/AddressSpace.js",
            "./WSL/AnonymousVariable.js",
            "./WSL/ArrayRefType.js",
            "./WSL/ArrayType.js",
            "./WSL/Assignment.js",
            "./WSL/AutoWrapper.js",
            "./WSL/Block.js",
            "./WSL/BoolLiteral.js",
            "./WSL/Break.js",
            "./WSL/CallExpression.js",
            "./WSL/CallFunction.js",
            "./WSL/Check.js",
            "./WSL/CheckLiteralTypes.js",
            "./WSL/CheckLoops.js",
            "./WSL/CheckRecursiveTypes.js",
            "./WSL/CheckRecursion.js",
            "./WSL/CheckReturns.js",
            "./WSL/CheckUnreachableCode.js",
            "./WSL/CheckWrapped.js",
            "./WSL/Checker.js",
            "./WSL/CloneProgram.js",
            "./WSL/CommaExpression.js",
            "./WSL/ConstexprFolder.js",
            "./WSL/ConstexprTypeParameter.js",
            "./WSL/Continue.js",
            "./WSL/ConvertPtrToArrayRefExpression.js",
            "./WSL/DereferenceExpression.js",
            "./WSL/DoWhileLoop.js",
            "./WSL/DotExpression.js",
            "./WSL/DoubleLiteral.js",
            "./WSL/DoubleLiteralType.js",
            "./WSL/EArrayRef.js",
            "./WSL/EBuffer.js",
            "./WSL/EBufferBuilder.js",
            "./WSL/EPtr.js",
            "./WSL/EnumLiteral.js",
            "./WSL/EnumMember.js",
            "./WSL/EnumType.js",
            "./WSL/EvaluationCommon.js",
            "./WSL/Evaluator.js",
            "./WSL/ExpressionFinder.js",
            "./WSL/ExternalOrigin.js",
            "./WSL/Field.js",
            "./WSL/FindHighZombies.js",
            "./WSL/FlattenProtocolExtends.js",
            "./WSL/FlattenedStructOffsetGatherer.js",
            "./WSL/FloatLiteral.js",
            "./WSL/FloatLiteralType.js",
            "./WSL/FoldConstexprs.js",
            "./WSL/ForLoop.js",
            "./WSL/Func.js",
            "./WSL/FuncDef.js",
            "./WSL/FuncInstantiator.js",
            "./WSL/FuncParameter.js",
            "./WSL/FunctionLikeBlock.js",
            "./WSL/HighZombieFinder.js",
            "./WSL/IdentityExpression.js",
            "./WSL/IfStatement.js",
            "./WSL/IndexExpression.js",
            "./WSL/InferTypesForCall.js",
            "./WSL/Inline.js",
            "./WSL/Inliner.js",
            "./WSL/InstantiateImmediates.js",
            "./WSL/IntLiteral.js",
            "./WSL/IntLiteralType.js",
            "./WSL/Intrinsics.js",
            "./WSL/LateChecker.js",
            "./WSL/Lexer.js",
            "./WSL/LexerToken.js",
            "./WSL/LiteralTypeChecker.js",
            "./WSL/LogicalExpression.js",
            "./WSL/LogicalNot.js",
            "./WSL/LoopChecker.js",
            "./WSL/MakeArrayRefExpression.js",
            "./WSL/MakePtrExpression.js",
            "./WSL/NameContext.js",
            "./WSL/NameFinder.js",
            "./WSL/NameResolver.js",
            "./WSL/NativeFunc.js",
            "./WSL/NativeFuncInstance.js",
            "./WSL/NativeType.js",
            "./WSL/NativeTypeInstance.js",
            "./WSL/NormalUsePropertyResolver.js",
            "./WSL/NullLiteral.js",
            "./WSL/NullType.js",
            "./WSL/OriginKind.js",
            "./WSL/OverloadResolutionFailure.js",
            "./WSL/Parse.js",
            "./WSL/Prepare.js",
            "./WSL/Program.js",
            "./WSL/ProgramWithUnnecessaryThingsRemoved.js",
            "./WSL/PropertyResolver.js",
            "./WSL/Protocol.js",
            "./WSL/ProtocolDecl.js",
            "./WSL/ProtocolFuncDecl.js",
            "./WSL/ProtocolRef.js",
            "./WSL/PtrType.js",
            "./WSL/ReadModifyWriteExpression.js",
            "./WSL/RecursionChecker.js",
            "./WSL/RecursiveTypeChecker.js",
            "./WSL/ResolveNames.js",
            "./WSL/ResolveOverloadImpl.js",
            "./WSL/ResolveProperties.js",
            "./WSL/ResolveTypeDefs.js",
            "./WSL/Return.js",
            "./WSL/ReturnChecker.js",
            "./WSL/ReturnException.js",
            "./WSL/StandardLibrary.js",
            "./WSL/StatementCloner.js",
            "./WSL/StructLayoutBuilder.js",
            "./WSL/StructType.js",
            "./WSL/Substitution.js",
            "./WSL/SwitchCase.js",
            "./WSL/SwitchStatement.js",
            "./WSL/SynthesizeEnumFunctions.js",
            "./WSL/SynthesizeStructAccessors.js",
            "./WSL/TrapStatement.js",
            "./WSL/TypeDef.js",
            "./WSL/TypeDefResolver.js",
            "./WSL/TypeOrVariableRef.js",
            "./WSL/TypeParameterRewriter.js",
            "./WSL/TypeRef.js",
            "./WSL/TypeVariable.js",
            "./WSL/TypeVariableTracker.js",
            "./WSL/TypedValue.js",
            "./WSL/UintLiteral.js",
            "./WSL/UintLiteralType.js",
            "./WSL/UnificationContext.js",
            "./WSL/UnreachableCodeChecker.js",
            "./WSL/VariableDecl.js",
            "./WSL/VariableRef.js",
            "./WSL/VisitingSet.js",
            "./WSL/WSyntaxError.js",
            "./WSL/WTrapError.js",
            "./WSL/WTypeError.js",
            "./WSL/WhileLoop.js",
            "./WSL/WrapChecker.js",
            "./WSL/Test.js",
        ],
        tags: ["default", "js", "WSL"],
    }),
    // 8bitbench
    new WasmEMCCBenchmark({
        name: "8bitbench-wasm",
        files: [
            "./utils/polyfills/fast-text-encoding/1.0.3/text.js",
            "./8bitbench/build/rust/pkg/emu_bench.js",
            "./8bitbench/benchmark.js",
        ],
        preload: {
            wasmBinary: "./8bitbench/build/rust/pkg/emu_bench_bg.wasm",
            romBinary: "./8bitbench/build/assets/program.bin",
        },
        iterations: 15,
        worstCaseCount: 2,
        tags: ["default", "Wasm"],
    }),
    // zlib-wasm
    new WasmEMCCBenchmark({
        name: "zlib-wasm",
        files: [
            "./wasm/zlib/build/zlib.js",
            "./wasm/zlib/benchmark.js",
        ],
        preload: {
            wasmBinary: "./wasm/zlib/build/zlib.wasm",
        },
        iterations: 40,
        tags: ["default", "Wasm"],
    }),
    // .NET
    new AsyncBenchmark({
        name: "dotnet-interp-wasm",
        files: [
            "./wasm/dotnet/interp.js",
            "./wasm/dotnet/benchmark.js",
        ],
        preload: dotnetPreloads("interp"),
        iterations: 10,
        worstCaseCount: 2,
        tags: ["default", "Wasm", "dotnet"],
    }),
    new AsyncBenchmark({
        name: "dotnet-aot-wasm",
        files: [
            "./wasm/dotnet/aot.js",
            "./wasm/dotnet/benchmark.js",
        ],
        preload: dotnetPreloads("aot"),
        iterations: 15,
        worstCaseCount: 2,
        tags: ["default", "Wasm", "dotnet"],
    }),
    // J2CL
    new AsyncBenchmark({
        name: "j2cl-box2d-wasm",
        files: [
            "./wasm/j2cl-box2d/benchmark.js",
            "./wasm/j2cl-box2d/build/Box2dBenchmark_j2wasm_entry.js",
        ],
        preload: {
            wasmBinary: "./wasm/j2cl-box2d/build/Box2dBenchmark_j2wasm_binary.wasm",
        },
        iterations: 40,
        tags: ["default", "Wasm"],
    }),
];


const PRISM_JS_FILES = [
    "./utils/StartupBenchmark.js",
    "./prismjs/benchmark.js",
];
const PRISM_JS_PRELOADS = {
    SAMPLE_CPP: "./prismjs/data/sample.cpp",
    SAMPLE_CSS: "./prismjs/data/sample.css",
    SAMPLE_HTML: "./prismjs/data/sample.html",
    SAMPLE_JS: "./prismjs/data/sample.js",
    SAMPLE_JSON: "./prismjs/data/sample.json",
    SAMPLE_MD: "./prismjs/data/sample.md",
    SAMPLE_PY: "./prismjs/data/sample.py",
    SAMPLE_SQL: "./prismjs/data/sample.sql",
    SAMPLE_TS: "./prismjs/data/sample.ts",
};
const PRISM_JS_TAGS = ["js", "parser", "regexp", "startup", "prismjs"];
BENCHMARKS.push(
    new AsyncBenchmark({
        name: "prismjs-startup-es6",
        files: PRISM_JS_FILES,
        preload: {
            // Use non-minified bundle for better local profiling.
            // BUNDLE: "./prismjs/dist/bundle.es6.js",
            BUNDLE: "./prismjs/dist/bundle.es6.min.js",
            ...PRISM_JS_PRELOADS,
        },
        tags: ["default", ...PRISM_JS_TAGS, "es6"],
    }),
    new AsyncBenchmark({
        name: "prismjs-startup-es5",
        files: PRISM_JS_FILES,
        preload: {
            // Use non-minified bundle for better local profiling.
            // BUNDLE: "./prismjs/dist/bundle.es5.js",
            BUNDLE: "./prismjs/dist/bundle.es5.min.js",
            ...PRISM_JS_PRELOADS,
        },
        tags: [...PRISM_JS_TAGS, "es5"],
    }),
);

const INTL_TESTS = [
    "DateTimeFormat",
    "ListFormat",
    "RelativeTimeFormat",
    "NumberFormat",
    "PluralRules",
];
const INTL_TAGS = ["js", "internationalization"]
const INTL_BENCHMARKS = [];
for (const test of INTL_TESTS) {
    const benchmark = new AsyncBenchmark({
        name: `${test}-intl`,
        files: [
            "./intl/src/helper.js",
            `./intl/src/${test}.js`,
            "./intl/benchmark.js",
        ],
        iterations: 2,
        worstCaseCount: 1,
        deterministicRandom: true,
        tags: INTL_TAGS,
    });
    INTL_BENCHMARKS.push(benchmark);
}
BENCHMARKS.push(
    new GroupedBenchmark({
            name: "intl",
            tags: INTL_TAGS,
        }, INTL_BENCHMARKS));



// SunSpider tests
const SUNSPIDER_TESTS = [
    "3d-cube",
    "3d-raytrace",
    "base64",
    "crypto-aes",
    "crypto-md5",
    "crypto-sha1",
    "date-format-tofte",
    "date-format-xparb",
    "n-body",
    "regex-dna",
    "string-unpack-code",
    "tagcloud",
];
let SUNSPIDER_BENCHMARKS = [];
for (const test of SUNSPIDER_TESTS) {
    SUNSPIDER_BENCHMARKS.push(new DefaultBenchmark({
        name: `${test}-SP`,
        files: [
            `./SunSpider/${test}.js`
        ],
        tags: [],
    }));
}
BENCHMARKS.push(new GroupedBenchmark({
    name: "Sunspider",
    tags: ["default", "js", "SunSpider"],
}, SUNSPIDER_BENCHMARKS))

// WTB (Web Tooling Benchmark) tests
const WTB_TESTS = {
    "acorn": true,
    "babel": true,
    "babel-minify": true,
    "babylon": true,
    "chai": true,
    "espree": true,
    "esprima-next": true,
    // Disabled: Converting ES5 code to ES6+ is no longer a realistic scenario.
    "lebab": false,
    "postcss": true,
    "prettier": true,
    "source-map": true,
};
const WPT_FILES = [
  "angular-material-20.1.6.css",
  "backbone-1.6.1.js",
  "bootstrap-5.3.7.css",
  "foundation-6.9.0.css",
  "jquery-3.7.1.js",
  "lodash.core-4.17.21.js",
  "lodash-4.17.4.min.js.map",
  "mootools-core-1.6.0.js",
  "preact-8.2.5.js",
  "preact-10.27.1.min.module.js.map",
  "redux-5.0.1.min.js",
  "redux-5.0.1.esm.js",
  "source-map.min-0.5.7.js.map",
  "source-map/lib/mappings.wasm",
  "speedometer-es2015-test-2.0.js",
  "todomvc/react/app.jsx",
  "todomvc/react/footer.jsx",
  "todomvc/react/todoItem.jsx",
  "todomvc/typescript-angular.ts",
  "underscore-1.13.7.js",
  "underscore-1.13.7.min.js.map",
  "vue-3.5.18.runtime.esm-browser.js",
].reduce((acc, file) => {
        acc[file] = `./web-tooling-benchmark/third_party/${file}`;
        return acc
}, Object.create(null));


for (const [name, enabled] of Object.entries(WTB_TESTS)) {
    const tags =  ["js", "WTB"];
    if (enabled)
        tags.push("Default");
    BENCHMARKS.push(new AsyncBenchmark({
        name: `${name}-wtb`,
        files: [
            `./web-tooling-benchmark/dist/${name}.bundle.js`,
            "./web-tooling-benchmark/benchmark.js",
        ],
        preload: {
            BUNDLE: `./web-tooling-benchmark/dist/${name}.bundle.js`,
            ...WPT_FILES,
        },
        iterations: 15,
        worstCaseCount: 2,
        allowUtf16: true,
        tags: tags,
    }));
}


const benchmarksByName = new Map();
const benchmarksByTag = new Map();

for (const benchmark of BENCHMARKS) {
    const name = benchmark.name.toLowerCase();

    if (benchmarksByName.has(name))
        throw new Error(`Duplicate benchmark with name "${name}}"`);
    else
        benchmarksByName.set(name, benchmark);

    for (const tag of benchmark.tags) {
        if (benchmarksByTag.has(tag))
            benchmarksByTag.get(tag).push(benchmark);
        else
            benchmarksByTag.set(tag, [benchmark]);
    }
}


function processTestList(testList) {
    let benchmarkNames = [];
    let benchmarks = [];

    if (testList instanceof Array)
        benchmarkNames = testList;
    else
        benchmarkNames = testList.split(/[\s,]/);

    for (let name of benchmarkNames) {
        name = name.toLowerCase();
        if (benchmarksByTag.has(name))
            benchmarks = benchmarks.concat(findBenchmarksByTag(name));
        else
            benchmarks.push(findBenchmarkByName(name));
    }
    return benchmarks;
}


function findBenchmarkByName(name) {
    const benchmark = benchmarksByName.get(name.toLowerCase());

    if (!benchmark)
        throw new Error(`Couldn't find benchmark named "${name}"`);

    return benchmark;
}


function findBenchmarksByTag(tag, excludeTags) {
    let benchmarks = benchmarksByTag.get(tag.toLowerCase());
    if (!benchmarks) {
        const validTags = Array.from(benchmarksByTag.keys()).join(", ");
        throw new Error(`Couldn't find tag named: ${tag}.\n Choices are ${validTags}`);
    }
    if (excludeTags) {
        benchmarks = benchmarks.filter(benchmark => {
            return !benchmark.hasAnyTag(...excludeTags);
        });
    }
    return benchmarks;
}


let benchmarks = [];
const defaultDisabledTags = [];
// FIXME: add better support to run Worker tests in shells.
if (!isInBrowser)
    defaultDisabledTags.push("WorkerTests");

if (JetStreamParams.testList.length) {
    benchmarks = processTestList(JetStreamParams.testList);
} else {
    benchmarks = findBenchmarksByTag("Default", defaultDisabledTags)
}

this.JetStream = new Driver(benchmarks);


JetStream.initialize()
    .then(() => JetStream.start())
    .then(() => print("JETSTREAM_RUN_COMPLETE"))
    .catch((error) => print("JetStream2 failed:", error && error.stack ? error.stack : error));
undefined;
