
const isInBrowser = false;
const jetStreamHostPrint = typeof globalThis.print === "function"
    ? globalThis.print
    : (...args) => globalThis.console.log(...args);
globalThis.print = jetStreamHostPrint;
var console = { log: (...args) => jetStreamHostPrint(...args) };
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
    testIterationCountMap: {},
    testWorstCaseCountMap: {},
    testList: "crypto",
};
var __jetstreamResources = {"./Octane/crypto.js":"/*\r\n * Copyright (c) 2003-2005  Tom Wu\r\n * All Rights Reserved.\r\n *\r\n * Permission is hereby granted, free of charge, to any person obtaining\r\n * a copy of this software and associated documentation files (the\r\n * \"Software\"), to deal in the Software without restriction, including\r\n * without limitation the rights to use, copy, modify, merge, publish,\r\n * distribute, sublicense, and/or sell copies of the Software, and to\r\n * permit persons to whom the Software is furnished to do so, subject to\r\n * the following conditions:\r\n *\r\n * The above copyright notice and this permission notice shall be\r\n * included in all copies or substantial portions of the Software.\r\n *\r\n * THE SOFTWARE IS PROVIDED \"AS-IS\" AND WITHOUT WARRANTY OF ANY KIND,\r\n * EXPRESS, IMPLIED OR OTHERWISE, INCLUDING WITHOUT LIMITATION, ANY\r\n * WARRANTY OF MERCHANTABILITY OR FITNESS FOR A PARTICULAR PURPOSE.\r\n *\r\n * IN NO EVENT SHALL TOM WU BE LIABLE FOR ANY SPECIAL, INCIDENTAL,\r\n * INDIRECT OR CONSEQUENTIAL DAMAGES OF ANY KIND, OR ANY DAMAGES WHATSOEVER\r\n * RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER OR NOT ADVISED OF\r\n * THE POSSIBILITY OF DAMAGE, AND ON ANY THEORY OF LIABILITY, ARISING OUT\r\n * OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.\r\n *\r\n * In addition, the following condition applies:\r\n *\r\n * All redistributions must retain an intact copy of this copyright notice\r\n * and disclaimer.\r\n */\r\n\r\n\r\n// The code has been adapted for use as a benchmark by Google.\r\n\r\n// Basic JavaScript BN library - subset useful for RSA encryption.\r\n\r\n// Bits per digit\r\nvar dbits;\r\nvar BI_DB;\r\nvar BI_DM;\r\nvar BI_DV;\r\n\r\nvar BI_FP;\r\nvar BI_FV;\r\nvar BI_F1;\r\nvar BI_F2;\r\n\r\n// JavaScript engine analysis\r\nvar canary = 0xdeadbeefcafe;\r\nvar j_lm = ((canary&0xffffff)==0xefcafe);\r\n\r\n// (public) Constructor\r\nfunction BigInteger(a,b,c) {\r\n  this.array = new Array();\r\n  if(a != null)\r\n    if(\"number\" == typeof a) this.fromNumber(a,b,c);\r\n    else if(b == null && \"string\" != typeof a) this.fromString(a,256);\r\n    else this.fromString(a,b);\r\n}\r\n\r\n// return new, unset BigInteger\r\nfunction nbi() { return new BigInteger(null); }\r\n\r\n// am: Compute w_j += (x*this_i), propagate carries,\r\n// c is initial carry, returns final carry.\r\n// c < 3*dvalue, x < 2*dvalue, this_i < dvalue\r\n// We need to select the fastest one that works in this environment.\r\n\r\n// am1: use a single mult and divide to get the high bits,\r\n// max digit bits should be 26 because\r\n// max internal value = 2*dvalue^2-2*dvalue (< 2^53)\r\nfunction am1(i,x,w,j,c,n) {\r\n  var this_array = this.array;\r\n  var w_array    = w.array;\r\n  while(--n >= 0) {\r\n    var v = x*this_array[i++]+w_array[j]+c;\r\n    c = Math.floor(v/0x4000000);\r\n    w_array[j++] = v&0x3ffffff;\r\n  }\r\n  return c;\r\n}\r\n\r\n// am2 avoids a big mult-and-extract completely.\r\n// Max digit bits should be <= 30 because we do bitwise ops\r\n// on values up to 2*hdvalue^2-hdvalue-1 (< 2^31)\r\nfunction am2(i,x,w,j,c,n) {\r\n  var this_array = this.array;\r\n  var w_array    = w.array;\r\n  var xl = x&0x7fff, xh = x>>15;\r\n  while(--n >= 0) {\r\n    var l = this_array[i]&0x7fff;\r\n    var h = this_array[i++]>>15;\r\n    var m = xh*l+h*xl;\r\n    l = xl*l+((m&0x7fff)<<15)+w_array[j]+(c&0x3fffffff);\r\n    c = (l>>>30)+(m>>>15)+xh*h+(c>>>30);\r\n    w_array[j++] = l&0x3fffffff;\r\n  }\r\n  return c;\r\n}\r\n\r\n// Alternately, set max digit bits to 28 since some\r\n// browsers slow down when dealing with 32-bit numbers.\r\nfunction am3(i,x,w,j,c,n) {\r\n  var this_array = this.array;\r\n  var w_array    = w.array;\r\n\r\n  var xl = x&0x3fff, xh = x>>14;\r\n  while(--n >= 0) {\r\n    var l = this_array[i]&0x3fff;\r\n    var h = this_array[i++]>>14;\r\n    var m = xh*l+h*xl;\r\n    l = xl*l+((m&0x3fff)<<14)+w_array[j]+c;\r\n    c = (l>>28)+(m>>14)+xh*h;\r\n    w_array[j++] = l&0xfffffff;\r\n  }\r\n  return c;\r\n}\r\n\r\n// This is tailored to VMs with 2-bit tagging. It makes sure\r\n// that all the computations stay within the 29 bits available.\r\nfunction am4(i,x,w,j,c,n) {\r\n  var this_array = this.array;\r\n  var w_array    = w.array;\r\n\r\n  var xl = x&0x1fff, xh = x>>13;\r\n  while(--n >= 0) {\r\n    var l = this_array[i]&0x1fff;\r\n    var h = this_array[i++]>>13;\r\n    var m = xh*l+h*xl;\r\n    l = xl*l+((m&0x1fff)<<13)+w_array[j]+c;\r\n    c = (l>>26)+(m>>13)+xh*h;\r\n    w_array[j++] = l&0x3ffffff;\r\n  }\r\n  return c;\r\n}\r\n\r\n// am3/28 is best for SM, Rhino, but am4/26 is best for v8.\r\n// Kestrel (Opera 9.5) gets its best result with am4/26.\r\n// IE7 does 9% better with am3/28 than with am4/26.\r\n// Firefox (SM) gets 10% faster with am3/28 than with am4/26.\r\n\r\nsetupEngine = function(fn, bits) {\r\n  BigInteger.prototype.am = fn;\r\n  dbits = bits;\r\n\r\n  BI_DB = dbits;\r\n  BI_DM = ((1<<dbits)-1);\r\n  BI_DV = (1<<dbits);\r\n\r\n  BI_FP = 52;\r\n  BI_FV = Math.pow(2,BI_FP);\r\n  BI_F1 = BI_FP-dbits;\r\n  BI_F2 = 2*dbits-BI_FP;\r\n}\r\n\r\n\r\n// Digit conversions\r\nvar BI_RM = \"0123456789abcdefghijklmnopqrstuvwxyz\";\r\nvar BI_RC = new Array();\r\nvar rr,vv;\r\nrr = \"0\".charCodeAt(0);\r\nfor(vv = 0; vv <= 9; ++vv) BI_RC[rr++] = vv;\r\nrr = \"a\".charCodeAt(0);\r\nfor(vv = 10; vv < 36; ++vv) BI_RC[rr++] = vv;\r\nrr = \"A\".charCodeAt(0);\r\nfor(vv = 10; vv < 36; ++vv) BI_RC[rr++] = vv;\r\n\r\nfunction int2char(n) { return BI_RM.charAt(n); }\r\nfunction intAt(s,i) {\r\n  var c = BI_RC[s.charCodeAt(i)];\r\n  return (c==null)?-1:c;\r\n}\r\n\r\n// (protected) copy this to r\r\nfunction bnpCopyTo(r) {\r\n  var this_array = this.array;\r\n  var r_array    = r.array;\r\n\r\n  for(var i = this.t-1; i >= 0; --i) r_array[i] = this_array[i];\r\n  r.t = this.t;\r\n  r.s = this.s;\r\n}\r\n\r\n// (protected) set from integer value x, -DV <= x < DV\r\nfunction bnpFromInt(x) {\r\n  var this_array = this.array;\r\n  this.t = 1;\r\n  this.s = (x<0)?-1:0;\r\n  if(x > 0) this_array[0] = x;\r\n  else if(x < -1) this_array[0] = x+DV;\r\n  else this.t = 0;\r\n}\r\n\r\n// return bigint initialized to value\r\nfunction nbv(i) { var r = nbi(); r.fromInt(i); return r; }\r\n\r\n// (protected) set from string and radix\r\nfunction bnpFromString(s,b) {\r\n  var this_array = this.array;\r\n  var k;\r\n  if(b == 16) k = 4;\r\n  else if(b == 8) k = 3;\r\n  else if(b == 256) k = 8; // byte array\r\n  else if(b == 2) k = 1;\r\n  else if(b == 32) k = 5;\r\n  else if(b == 4) k = 2;\r\n  else { this.fromRadix(s,b); return; }\r\n  this.t = 0;\r\n  this.s = 0;\r\n  var i = s.length, mi = false, sh = 0;\r\n  while(--i >= 0) {\r\n    var x = (k==8)?s[i]&0xff:intAt(s,i);\r\n    if(x < 0) {\r\n      if(s.charAt(i) == \"-\") mi = true;\r\n      continue;\r\n    }\r\n    mi = false;\r\n    if(sh == 0)\r\n      this_array[this.t++] = x;\r\n    else if(sh+k > BI_DB) {\r\n      this_array[this.t-1] |= (x&((1<<(BI_DB-sh))-1))<<sh;\r\n      this_array[this.t++] = (x>>(BI_DB-sh));\r\n    }\r\n    else\r\n      this_array[this.t-1] |= x<<sh;\r\n    sh += k;\r\n    if(sh >= BI_DB) sh -= BI_DB;\r\n  }\r\n  if(k == 8 && (s[0]&0x80) != 0) {\r\n    this.s = -1;\r\n    if(sh > 0) this_array[this.t-1] |= ((1<<(BI_DB-sh))-1)<<sh;\r\n  }\r\n  this.clamp();\r\n  if(mi) BigInteger.ZERO.subTo(this,this);\r\n}\r\n\r\n// (protected) clamp off excess high words\r\nfunction bnpClamp() {\r\n  var this_array = this.array;\r\n  var c = this.s&BI_DM;\r\n  while(this.t > 0 && this_array[this.t-1] == c) --this.t;\r\n}\r\n\r\n// (public) return string representation in given radix\r\nfunction bnToString(b) {\r\n  var this_array = this.array;\r\n  if(this.s < 0) return \"-\"+this.negate().toString(b);\r\n  var k;\r\n  if(b == 16) k = 4;\r\n  else if(b == 8) k = 3;\r\n  else if(b == 2) k = 1;\r\n  else if(b == 32) k = 5;\r\n  else if(b == 4) k = 2;\r\n  else return this.toRadix(b);\r\n  var km = (1<<k)-1, d, m = false, r = \"\", i = this.t;\r\n  var p = BI_DB-(i*BI_DB)%k;\r\n  if(i-- > 0) {\r\n    if(p < BI_DB && (d = this_array[i]>>p) > 0) { m = true; r = int2char(d); }\r\n    while(i >= 0) {\r\n      if(p < k) {\r\n        d = (this_array[i]&((1<<p)-1))<<(k-p);\r\n        d |= this_array[--i]>>(p+=BI_DB-k);\r\n      }\r\n      else {\r\n        d = (this_array[i]>>(p-=k))&km;\r\n        if(p <= 0) { p += BI_DB; --i; }\r\n      }\r\n      if(d > 0) m = true;\r\n      if(m) r += int2char(d);\r\n    }\r\n  }\r\n  return m?r:\"0\";\r\n}\r\n\r\n// (public) -this\r\nfunction bnNegate() { var r = nbi(); BigInteger.ZERO.subTo(this,r); return r; }\r\n\r\n// (public) |this|\r\nfunction bnAbs() { return (this.s<0)?this.negate():this; }\r\n\r\n// (public) return + if this > a, - if this < a, 0 if equal\r\nfunction bnCompareTo(a) {\r\n  var this_array = this.array;\r\n  var a_array = a.array;\r\n\r\n  var r = this.s-a.s;\r\n  if(r != 0) return r;\r\n  var i = this.t;\r\n  r = i-a.t;\r\n  if(r != 0) return r;\r\n  while(--i >= 0) if((r=this_array[i]-a_array[i]) != 0) return r;\r\n  return 0;\r\n}\r\n\r\n// returns bit length of the integer x\r\nfunction nbits(x) {\r\n  var r = 1, t;\r\n  if((t=x>>>16) != 0) { x = t; r += 16; }\r\n  if((t=x>>8) != 0) { x = t; r += 8; }\r\n  if((t=x>>4) != 0) { x = t; r += 4; }\r\n  if((t=x>>2) != 0) { x = t; r += 2; }\r\n  if((t=x>>1) != 0) { x = t; r += 1; }\r\n  return r;\r\n}\r\n\r\n// (public) return the number of bits in \"this\"\r\nfunction bnBitLength() {\r\n  var this_array = this.array;\r\n  if(this.t <= 0) return 0;\r\n  return BI_DB*(this.t-1)+nbits(this_array[this.t-1]^(this.s&BI_DM));\r\n}\r\n\r\n// (protected) r = this << n*DB\r\nfunction bnpDLShiftTo(n,r) {\r\n  var this_array = this.array;\r\n  var r_array = r.array;\r\n  var i;\r\n  for(i = this.t-1; i >= 0; --i) r_array[i+n] = this_array[i];\r\n  for(i = n-1; i >= 0; --i) r_array[i] = 0;\r\n  r.t = this.t+n;\r\n  r.s = this.s;\r\n}\r\n\r\n// (protected) r = this >> n*DB\r\nfunction bnpDRShiftTo(n,r) {\r\n  var this_array = this.array;\r\n  var r_array = r.array;\r\n  for(var i = n; i < this.t; ++i) r_array[i-n] = this_array[i];\r\n  r.t = Math.max(this.t-n,0);\r\n  r.s = this.s;\r\n}\r\n\r\n// (protected) r = this << n\r\nfunction bnpLShiftTo(n,r) {\r\n  var this_array = this.array;\r\n  var r_array = r.array;\r\n  var bs = n%BI_DB;\r\n  var cbs = BI_DB-bs;\r\n  var bm = (1<<cbs)-1;\r\n  var ds = Math.floor(n/BI_DB), c = (this.s<<bs)&BI_DM, i;\r\n  for(i = this.t-1; i >= 0; --i) {\r\n    r_array[i+ds+1] = (this_array[i]>>cbs)|c;\r\n    c = (this_array[i]&bm)<<bs;\r\n  }\r\n  for(i = ds-1; i >= 0; --i) r_array[i] = 0;\r\n  r_array[ds] = c;\r\n  r.t = this.t+ds+1;\r\n  r.s = this.s;\r\n  r.clamp();\r\n}\r\n\r\n// (protected) r = this >> n\r\nfunction bnpRShiftTo(n,r) {\r\n  var this_array = this.array;\r\n  var r_array = r.array;\r\n  r.s = this.s;\r\n  var ds = Math.floor(n/BI_DB);\r\n  if(ds >= this.t) { r.t = 0; return; }\r\n  var bs = n%BI_DB;\r\n  var cbs = BI_DB-bs;\r\n  var bm = (1<<bs)-1;\r\n  r_array[0] = this_array[ds]>>bs;\r\n  for(var i = ds+1; i < this.t; ++i) {\r\n    r_array[i-ds-1] |= (this_array[i]&bm)<<cbs;\r\n    r_array[i-ds] = this_array[i]>>bs;\r\n  }\r\n  if(bs > 0) r_array[this.t-ds-1] |= (this.s&bm)<<cbs;\r\n  r.t = this.t-ds;\r\n  r.clamp();\r\n}\r\n\r\n// (protected) r = this - a\r\nfunction bnpSubTo(a,r) {\r\n  var this_array = this.array;\r\n  var r_array = r.array;\r\n  var a_array = a.array;\r\n  var i = 0, c = 0, m = Math.min(a.t,this.t);\r\n  while(i < m) {\r\n    c += this_array[i]-a_array[i];\r\n    r_array[i++] = c&BI_DM;\r\n    c >>= BI_DB;\r\n  }\r\n  if(a.t < this.t) {\r\n    c -= a.s;\r\n    while(i < this.t) {\r\n      c += this_array[i];\r\n      r_array[i++] = c&BI_DM;\r\n      c >>= BI_DB;\r\n    }\r\n    c += this.s;\r\n  }\r\n  else {\r\n    c += this.s;\r\n    while(i < a.t) {\r\n      c -= a_array[i];\r\n      r_array[i++] = c&BI_DM;\r\n      c >>= BI_DB;\r\n    }\r\n    c -= a.s;\r\n  }\r\n  r.s = (c<0)?-1:0;\r\n  if(c < -1) r_array[i++] = BI_DV+c;\r\n  else if(c > 0) r_array[i++] = c;\r\n  r.t = i;\r\n  r.clamp();\r\n}\r\n\r\n// (protected) r = this * a, r != this,a (HAC 14.12)\r\n// \"this\" should be the larger one if appropriate.\r\nfunction bnpMultiplyTo(a,r) {\r\n  var this_array = this.array;\r\n  var r_array = r.array;\r\n  var x = this.abs(), y = a.abs();\r\n  var y_array = y.array;\r\n\r\n  var i = x.t;\r\n  r.t = i+y.t;\r\n  while(--i >= 0) r_array[i] = 0;\r\n  for(i = 0; i < y.t; ++i) r_array[i+x.t] = x.am(0,y_array[i],r,i,0,x.t);\r\n  r.s = 0;\r\n  r.clamp();\r\n  if(this.s != a.s) BigInteger.ZERO.subTo(r,r);\r\n}\r\n\r\n// (protected) r = this^2, r != this (HAC 14.16)\r\nfunction bnpSquareTo(r) {\r\n  var x = this.abs();\r\n  var x_array = x.array;\r\n  var r_array = r.array;\r\n\r\n  var i = r.t = 2*x.t;\r\n  while(--i >= 0) r_array[i] = 0;\r\n  for(i = 0; i < x.t-1; ++i) {\r\n    var c = x.am(i,x_array[i],r,2*i,0,1);\r\n    if((r_array[i+x.t]+=x.am(i+1,2*x_array[i],r,2*i+1,c,x.t-i-1)) >= BI_DV) {\r\n      r_array[i+x.t] -= BI_DV;\r\n      r_array[i+x.t+1] = 1;\r\n    }\r\n  }\r\n  if(r.t > 0) r_array[r.t-1] += x.am(i,x_array[i],r,2*i,0,1);\r\n  r.s = 0;\r\n  r.clamp();\r\n}\r\n\r\n// (protected) divide this by m, quotient and remainder to q, r (HAC 14.20)\r\n// r != q, this != m.  q or r may be null.\r\nfunction bnpDivRemTo(m,q,r) {\r\n  var pm = m.abs();\r\n  if(pm.t <= 0) return;\r\n  var pt = this.abs();\r\n  if(pt.t < pm.t) {\r\n    if(q != null) q.fromInt(0);\r\n    if(r != null) this.copyTo(r);\r\n    return;\r\n  }\r\n  if(r == null) r = nbi();\r\n  var y = nbi(), ts = this.s, ms = m.s;\r\n  var pm_array = pm.array;\r\n  var nsh = BI_DB-nbits(pm_array[pm.t-1]);\t// normalize modulus\r\n  if(nsh > 0) { pm.lShiftTo(nsh,y); pt.lShiftTo(nsh,r); }\r\n  else { pm.copyTo(y); pt.copyTo(r); }\r\n  var ys = y.t;\r\n\r\n  var y_array = y.array;\r\n  var y0 = y_array[ys-1];\r\n  if(y0 == 0) return;\r\n  var yt = y0*(1<<BI_F1)+((ys>1)?y_array[ys-2]>>BI_F2:0);\r\n  var d1 = BI_FV/yt, d2 = (1<<BI_F1)/yt, e = 1<<BI_F2;\r\n  var i = r.t, j = i-ys, t = (q==null)?nbi():q;\r\n  y.dlShiftTo(j,t);\r\n\r\n  var r_array = r.array;\r\n  if(r.compareTo(t) >= 0) {\r\n    r_array[r.t++] = 1;\r\n    r.subTo(t,r);\r\n  }\r\n  BigInteger.ONE.dlShiftTo(ys,t);\r\n  t.subTo(y,y);\t// \"negative\" y so we can replace sub with am later\r\n  while(y.t < ys) y_array[y.t++] = 0;\r\n  while(--j >= 0) {\r\n    // Estimate quotient digit\r\n    var qd = (r_array[--i]==y0)?BI_DM:Math.floor(r_array[i]*d1+(r_array[i-1]+e)*d2);\r\n    if((r_array[i]+=y.am(0,qd,r,j,0,ys)) < qd) {\t// Try it out\r\n      y.dlShiftTo(j,t);\r\n      r.subTo(t,r);\r\n      while(r_array[i] < --qd) r.subTo(t,r);\r\n    }\r\n  }\r\n  if(q != null) {\r\n    r.drShiftTo(ys,q);\r\n    if(ts != ms) BigInteger.ZERO.subTo(q,q);\r\n  }\r\n  r.t = ys;\r\n  r.clamp();\r\n  if(nsh > 0) r.rShiftTo(nsh,r);\t// Denormalize remainder\r\n  if(ts < 0) BigInteger.ZERO.subTo(r,r);\r\n}\r\n\r\n// (public) this mod a\r\nfunction bnMod(a) {\r\n  var r = nbi();\r\n  this.abs().divRemTo(a,null,r);\r\n  if(this.s < 0 && r.compareTo(BigInteger.ZERO) > 0) a.subTo(r,r);\r\n  return r;\r\n}\r\n\r\n// Modular reduction using \"classic\" algorithm\r\nfunction Classic(m) { this.m = m; }\r\nfunction cConvert(x) {\r\n  if(x.s < 0 || x.compareTo(this.m) >= 0) return x.mod(this.m);\r\n  else return x;\r\n}\r\nfunction cRevert(x) { return x; }\r\nfunction cReduce(x) { x.divRemTo(this.m,null,x); }\r\nfunction cMulTo(x,y,r) { x.multiplyTo(y,r); this.reduce(r); }\r\nfunction cSqrTo(x,r) { x.squareTo(r); this.reduce(r); }\r\n\r\nClassic.prototype.convert = cConvert;\r\nClassic.prototype.revert = cRevert;\r\nClassic.prototype.reduce = cReduce;\r\nClassic.prototype.mulTo = cMulTo;\r\nClassic.prototype.sqrTo = cSqrTo;\r\n\r\n// (protected) return \"-1/this % 2^DB\"; useful for Mont. reduction\r\n// justification:\r\n//         xy == 1 (mod m)\r\n//         xy =  1+km\r\n//   xy(2-xy) = (1+km)(1-km)\r\n// x[y(2-xy)] = 1-k^2m^2\r\n// x[y(2-xy)] == 1 (mod m^2)\r\n// if y is 1/x mod m, then y(2-xy) is 1/x mod m^2\r\n// should reduce x and y(2-xy) by m^2 at each step to keep size bounded.\r\n// JS multiply \"overflows\" differently from C/C++, so care is needed here.\r\nfunction bnpInvDigit() {\r\n  var this_array = this.array;\r\n  if(this.t < 1) return 0;\r\n  var x = this_array[0];\r\n  if((x&1) == 0) return 0;\r\n  var y = x&3;\t\t// y == 1/x mod 2^2\r\n  y = (y*(2-(x&0xf)*y))&0xf;\t// y == 1/x mod 2^4\r\n  y = (y*(2-(x&0xff)*y))&0xff;\t// y == 1/x mod 2^8\r\n  y = (y*(2-(((x&0xffff)*y)&0xffff)))&0xffff;\t// y == 1/x mod 2^16\r\n  // last step - calculate inverse mod DV directly;\r\n  // assumes 16 < DB <= 32 and assumes ability to handle 48-bit ints\r\n  y = (y*(2-x*y%BI_DV))%BI_DV;\t\t// y == 1/x mod 2^dbits\r\n  // we really want the negative inverse, and -DV < y < DV\r\n  return (y>0)?BI_DV-y:-y;\r\n}\r\n\r\n// Montgomery reduction\r\nfunction Montgomery(m) {\r\n  this.m = m;\r\n  this.mp = m.invDigit();\r\n  this.mpl = this.mp&0x7fff;\r\n  this.mph = this.mp>>15;\r\n  this.um = (1<<(BI_DB-15))-1;\r\n  this.mt2 = 2*m.t;\r\n}\r\n\r\n// xR mod m\r\nfunction montConvert(x) {\r\n  var r = nbi();\r\n  x.abs().dlShiftTo(this.m.t,r);\r\n  r.divRemTo(this.m,null,r);\r\n  if(x.s < 0 && r.compareTo(BigInteger.ZERO) > 0) this.m.subTo(r,r);\r\n  return r;\r\n}\r\n\r\n// x/R mod m\r\nfunction montRevert(x) {\r\n  var r = nbi();\r\n  x.copyTo(r);\r\n  this.reduce(r);\r\n  return r;\r\n}\r\n\r\n// x = x/R mod m (HAC 14.32)\r\nfunction montReduce(x) {\r\n  var x_array = x.array;\r\n  while(x.t <= this.mt2)\t// pad x so am has enough room later\r\n    x_array[x.t++] = 0;\r\n  for(var i = 0; i < this.m.t; ++i) {\r\n    // faster way of calculating u0 = x[i]*mp mod DV\r\n    var j = x_array[i]&0x7fff;\r\n    var u0 = (j*this.mpl+(((j*this.mph+(x_array[i]>>15)*this.mpl)&this.um)<<15))&BI_DM;\r\n    // use am to combine the multiply-shift-add into one call\r\n    j = i+this.m.t;\r\n    x_array[j] += this.m.am(0,u0,x,i,0,this.m.t);\r\n    // propagate carry\r\n    while(x_array[j] >= BI_DV) { x_array[j] -= BI_DV; x_array[++j]++; }\r\n  }\r\n  x.clamp();\r\n  x.drShiftTo(this.m.t,x);\r\n  if(x.compareTo(this.m) >= 0) x.subTo(this.m,x);\r\n}\r\n\r\n// r = \"x^2/R mod m\"; x != r\r\nfunction montSqrTo(x,r) { x.squareTo(r); this.reduce(r); }\r\n\r\n// r = \"xy/R mod m\"; x,y != r\r\nfunction montMulTo(x,y,r) { x.multiplyTo(y,r); this.reduce(r); }\r\n\r\nMontgomery.prototype.convert = montConvert;\r\nMontgomery.prototype.revert = montRevert;\r\nMontgomery.prototype.reduce = montReduce;\r\nMontgomery.prototype.mulTo = montMulTo;\r\nMontgomery.prototype.sqrTo = montSqrTo;\r\n\r\n// (protected) true iff this is even\r\nfunction bnpIsEven() {\r\n  var this_array = this.array;\r\n  return ((this.t>0)?(this_array[0]&1):this.s) == 0;\r\n}\r\n\r\n// (protected) this^e, e < 2^32, doing sqr and mul with \"r\" (HAC 14.79)\r\nfunction bnpExp(e,z) {\r\n  if(e > 0xffffffff || e < 1) return BigInteger.ONE;\r\n  var r = nbi(), r2 = nbi(), g = z.convert(this), i = nbits(e)-1;\r\n  g.copyTo(r);\r\n  while(--i >= 0) {\r\n    z.sqrTo(r,r2);\r\n    if((e&(1<<i)) > 0) z.mulTo(r2,g,r);\r\n    else { var t = r; r = r2; r2 = t; }\r\n  }\r\n  return z.revert(r);\r\n}\r\n\r\n// (public) this^e % m, 0 <= e < 2^32\r\nfunction bnModPowInt(e,m) {\r\n  var z;\r\n  if(e < 256 || m.isEven()) z = new Classic(m); else z = new Montgomery(m);\r\n  return this.exp(e,z);\r\n}\r\n\r\n// protected\r\nBigInteger.prototype.copyTo = bnpCopyTo;\r\nBigInteger.prototype.fromInt = bnpFromInt;\r\nBigInteger.prototype.fromString = bnpFromString;\r\nBigInteger.prototype.clamp = bnpClamp;\r\nBigInteger.prototype.dlShiftTo = bnpDLShiftTo;\r\nBigInteger.prototype.drShiftTo = bnpDRShiftTo;\r\nBigInteger.prototype.lShiftTo = bnpLShiftTo;\r\nBigInteger.prototype.rShiftTo = bnpRShiftTo;\r\nBigInteger.prototype.subTo = bnpSubTo;\r\nBigInteger.prototype.multiplyTo = bnpMultiplyTo;\r\nBigInteger.prototype.squareTo = bnpSquareTo;\r\nBigInteger.prototype.divRemTo = bnpDivRemTo;\r\nBigInteger.prototype.invDigit = bnpInvDigit;\r\nBigInteger.prototype.isEven = bnpIsEven;\r\nBigInteger.prototype.exp = bnpExp;\r\n\r\n// public\r\nBigInteger.prototype.toString = bnToString;\r\nBigInteger.prototype.negate = bnNegate;\r\nBigInteger.prototype.abs = bnAbs;\r\nBigInteger.prototype.compareTo = bnCompareTo;\r\nBigInteger.prototype.bitLength = bnBitLength;\r\nBigInteger.prototype.mod = bnMod;\r\nBigInteger.prototype.modPowInt = bnModPowInt;\r\n\r\n// \"constants\"\r\nBigInteger.ZERO = nbv(0);\r\nBigInteger.ONE = nbv(1);\r\n// Copyright (c) 2005  Tom Wu\r\n// All Rights Reserved.\r\n// See \"LICENSE\" for details.\r\n\r\n// Extended JavaScript BN functions, required for RSA private ops.\r\n\r\n// (public)\r\nfunction bnClone() { var r = nbi(); this.copyTo(r); return r; }\r\n\r\n// (public) return value as integer\r\nfunction bnIntValue() {\r\n  var this_array = this.array;\r\n  if(this.s < 0) {\r\n    if(this.t == 1) return this_array[0]-BI_DV;\r\n    else if(this.t == 0) return -1;\r\n  }\r\n  else if(this.t == 1) return this_array[0];\r\n  else if(this.t == 0) return 0;\r\n  // assumes 16 < DB < 32\r\n  return ((this_array[1]&((1<<(32-BI_DB))-1))<<BI_DB)|this_array[0];\r\n}\r\n\r\n// (public) return value as byte\r\nfunction bnByteValue() {\r\n  var this_array = this.array;\r\n  return (this.t==0)?this.s:(this_array[0]<<24)>>24;\r\n}\r\n\r\n// (public) return value as short (assumes DB>=16)\r\nfunction bnShortValue() {\r\n  var this_array = this.array;\r\n  return (this.t==0)?this.s:(this_array[0]<<16)>>16;\r\n}\r\n\r\n// (protected) return x s.t. r^x < DV\r\nfunction bnpChunkSize(r) { return Math.floor(Math.LN2*BI_DB/Math.log(r)); }\r\n\r\n// (public) 0 if this == 0, 1 if this > 0\r\nfunction bnSigNum() {\r\n  var this_array = this.array;\r\n  if(this.s < 0) return -1;\r\n  else if(this.t <= 0 || (this.t == 1 && this_array[0] <= 0)) return 0;\r\n  else return 1;\r\n}\r\n\r\n// (protected) convert to radix string\r\nfunction bnpToRadix(b) {\r\n  if(b == null) b = 10;\r\n  if(this.signum() == 0 || b < 2 || b > 36) return \"0\";\r\n  var cs = this.chunkSize(b);\r\n  var a = Math.pow(b,cs);\r\n  var d = nbv(a), y = nbi(), z = nbi(), r = \"\";\r\n  this.divRemTo(d,y,z);\r\n  while(y.signum() > 0) {\r\n    r = (a+z.intValue()).toString(b).substr(1) + r;\r\n    y.divRemTo(d,y,z);\r\n  }\r\n  return z.intValue().toString(b) + r;\r\n}\r\n\r\n// (protected) convert from radix string\r\nfunction bnpFromRadix(s,b) {\r\n  this.fromInt(0);\r\n  if(b == null) b = 10;\r\n  var cs = this.chunkSize(b);\r\n  var d = Math.pow(b,cs), mi = false, j = 0, w = 0;\r\n  for(var i = 0; i < s.length; ++i) {\r\n    var x = intAt(s,i);\r\n    if(x < 0) {\r\n      if(s.charAt(i) == \"-\" && this.signum() == 0) mi = true;\r\n      continue;\r\n    }\r\n    w = b*w+x;\r\n    if(++j >= cs) {\r\n      this.dMultiply(d);\r\n      this.dAddOffset(w,0);\r\n      j = 0;\r\n      w = 0;\r\n    }\r\n  }\r\n  if(j > 0) {\r\n    this.dMultiply(Math.pow(b,j));\r\n    this.dAddOffset(w,0);\r\n  }\r\n  if(mi) BigInteger.ZERO.subTo(this,this);\r\n}\r\n\r\n// (protected) alternate constructor\r\nfunction bnpFromNumber(a,b,c) {\r\n  if(\"number\" == typeof b) {\r\n    // new BigInteger(int,int,RNG)\r\n    if(a < 2) this.fromInt(1);\r\n    else {\r\n      this.fromNumber(a,c);\r\n      if(!this.testBit(a-1))\t// force MSB set\r\n        this.bitwiseTo(BigInteger.ONE.shiftLeft(a-1),op_or,this);\r\n      if(this.isEven()) this.dAddOffset(1,0); // force odd\r\n      while(!this.isProbablePrime(b)) {\r\n        this.dAddOffset(2,0);\r\n        if(this.bitLength() > a) this.subTo(BigInteger.ONE.shiftLeft(a-1),this);\r\n      }\r\n    }\r\n  }\r\n  else {\r\n    // new BigInteger(int,RNG)\r\n    var x = new Array(), t = a&7;\r\n    x.length = (a>>3)+1;\r\n    b.nextBytes(x);\r\n    if(t > 0) x[0] &= ((1<<t)-1); else x[0] = 0;\r\n    this.fromString(x,256);\r\n  }\r\n}\r\n\r\n// (public) convert to bigendian byte array\r\nfunction bnToByteArray() {\r\n  var this_array = this.array;\r\n  var i = this.t, r = new Array();\r\n  r[0] = this.s;\r\n  var p = BI_DB-(i*BI_DB)%8, d, k = 0;\r\n  if(i-- > 0) {\r\n    if(p < BI_DB && (d = this_array[i]>>p) != (this.s&BI_DM)>>p)\r\n      r[k++] = d|(this.s<<(BI_DB-p));\r\n    while(i >= 0) {\r\n      if(p < 8) {\r\n        d = (this_array[i]&((1<<p)-1))<<(8-p);\r\n        d |= this_array[--i]>>(p+=BI_DB-8);\r\n      }\r\n      else {\r\n        d = (this_array[i]>>(p-=8))&0xff;\r\n        if(p <= 0) { p += BI_DB; --i; }\r\n      }\r\n      if((d&0x80) != 0) d |= -256;\r\n      if(k == 0 && (this.s&0x80) != (d&0x80)) ++k;\r\n      if(k > 0 || d != this.s) r[k++] = d;\r\n    }\r\n  }\r\n  return r;\r\n}\r\n\r\nfunction bnEquals(a) { return(this.compareTo(a)==0); }\r\nfunction bnMin(a) { return(this.compareTo(a)<0)?this:a; }\r\nfunction bnMax(a) { return(this.compareTo(a)>0)?this:a; }\r\n\r\n// (protected) r = this op a (bitwise)\r\nfunction bnpBitwiseTo(a,op,r) {\r\n  var this_array = this.array;\r\n  var a_array    = a.array;\r\n  var r_array    = r.array;\r\n  var i, f, m = Math.min(a.t,this.t);\r\n  for(i = 0; i < m; ++i) r_array[i] = op(this_array[i],a_array[i]);\r\n  if(a.t < this.t) {\r\n    f = a.s&BI_DM;\r\n    for(i = m; i < this.t; ++i) r_array[i] = op(this_array[i],f);\r\n    r.t = this.t;\r\n  }\r\n  else {\r\n    f = this.s&BI_DM;\r\n    for(i = m; i < a.t; ++i) r_array[i] = op(f,a_array[i]);\r\n    r.t = a.t;\r\n  }\r\n  r.s = op(this.s,a.s);\r\n  r.clamp();\r\n}\r\n\r\n// (public) this & a\r\nfunction op_and(x,y) { return x&y; }\r\nfunction bnAnd(a) { var r = nbi(); this.bitwiseTo(a,op_and,r); return r; }\r\n\r\n// (public) this | a\r\nfunction op_or(x,y) { return x|y; }\r\nfunction bnOr(a) { var r = nbi(); this.bitwiseTo(a,op_or,r); return r; }\r\n\r\n// (public) this ^ a\r\nfunction op_xor(x,y) { return x^y; }\r\nfunction bnXor(a) { var r = nbi(); this.bitwiseTo(a,op_xor,r); return r; }\r\n\r\n// (public) this & ~a\r\nfunction op_andnot(x,y) { return x&~y; }\r\nfunction bnAndNot(a) { var r = nbi(); this.bitwiseTo(a,op_andnot,r); return r; }\r\n\r\n// (public) ~this\r\nfunction bnNot() {\r\n  var this_array = this.array;\r\n  var r = nbi();\r\n  var r_array = r.array;\r\n\r\n  for(var i = 0; i < this.t; ++i) r_array[i] = BI_DM&~this_array[i];\r\n  r.t = this.t;\r\n  r.s = ~this.s;\r\n  return r;\r\n}\r\n\r\n// (public) this << n\r\nfunction bnShiftLeft(n) {\r\n  var r = nbi();\r\n  if(n < 0) this.rShiftTo(-n,r); else this.lShiftTo(n,r);\r\n  return r;\r\n}\r\n\r\n// (public) this >> n\r\nfunction bnShiftRight(n) {\r\n  var r = nbi();\r\n  if(n < 0) this.lShiftTo(-n,r); else this.rShiftTo(n,r);\r\n  return r;\r\n}\r\n\r\n// return index of lowest 1-bit in x, x < 2^31\r\nfunction lbit(x) {\r\n  if(x == 0) return -1;\r\n  var r = 0;\r\n  if((x&0xffff) == 0) { x >>= 16; r += 16; }\r\n  if((x&0xff) == 0) { x >>= 8; r += 8; }\r\n  if((x&0xf) == 0) { x >>= 4; r += 4; }\r\n  if((x&3) == 0) { x >>= 2; r += 2; }\r\n  if((x&1) == 0) ++r;\r\n  return r;\r\n}\r\n\r\n// (public) returns index of lowest 1-bit (or -1 if none)\r\nfunction bnGetLowestSetBit() {\r\n  var this_array = this.array;\r\n  for(var i = 0; i < this.t; ++i)\r\n    if(this_array[i] != 0) return i*BI_DB+lbit(this_array[i]);\r\n  if(this.s < 0) return this.t*BI_DB;\r\n  return -1;\r\n}\r\n\r\n// return number of 1 bits in x\r\nfunction cbit(x) {\r\n  var r = 0;\r\n  while(x != 0) { x &= x-1; ++r; }\r\n  return r;\r\n}\r\n\r\n// (public) return number of set bits\r\nfunction bnBitCount() {\r\n  var r = 0, x = this.s&BI_DM;\r\n  for(var i = 0; i < this.t; ++i) r += cbit(this_array[i]^x);\r\n  return r;\r\n}\r\n\r\n// (public) true iff nth bit is set\r\nfunction bnTestBit(n) {\r\n  var this_array = this.array;\r\n  var j = Math.floor(n/BI_DB);\r\n  if(j >= this.t) return(this.s!=0);\r\n  return((this_array[j]&(1<<(n%BI_DB)))!=0);\r\n}\r\n\r\n// (protected) this op (1<<n)\r\nfunction bnpChangeBit(n,op) {\r\n  var r = BigInteger.ONE.shiftLeft(n);\r\n  this.bitwiseTo(r,op,r);\r\n  return r;\r\n}\r\n\r\n// (public) this | (1<<n)\r\nfunction bnSetBit(n) { return this.changeBit(n,op_or); }\r\n\r\n// (public) this & ~(1<<n)\r\nfunction bnClearBit(n) { return this.changeBit(n,op_andnot); }\r\n\r\n// (public) this ^ (1<<n)\r\nfunction bnFlipBit(n) { return this.changeBit(n,op_xor); }\r\n\r\n// (protected) r = this + a\r\nfunction bnpAddTo(a,r) {\r\n  var this_array = this.array;\r\n  var a_array = a.array;\r\n  var r_array = r.array;\r\n  var i = 0, c = 0, m = Math.min(a.t,this.t);\r\n  while(i < m) {\r\n    c += this_array[i]+a_array[i];\r\n    r_array[i++] = c&BI_DM;\r\n    c >>= BI_DB;\r\n  }\r\n  if(a.t < this.t) {\r\n    c += a.s;\r\n    while(i < this.t) {\r\n      c += this_array[i];\r\n      r_array[i++] = c&BI_DM;\r\n      c >>= BI_DB;\r\n    }\r\n    c += this.s;\r\n  }\r\n  else {\r\n    c += this.s;\r\n    while(i < a.t) {\r\n      c += a_array[i];\r\n      r_array[i++] = c&BI_DM;\r\n      c >>= BI_DB;\r\n    }\r\n    c += a.s;\r\n  }\r\n  r.s = (c<0)?-1:0;\r\n  if(c > 0) r_array[i++] = c;\r\n  else if(c < -1) r_array[i++] = BI_DV+c;\r\n  r.t = i;\r\n  r.clamp();\r\n}\r\n\r\n// (public) this + a\r\nfunction bnAdd(a) { var r = nbi(); this.addTo(a,r); return r; }\r\n\r\n// (public) this - a\r\nfunction bnSubtract(a) { var r = nbi(); this.subTo(a,r); return r; }\r\n\r\n// (public) this * a\r\nfunction bnMultiply(a) { var r = nbi(); this.multiplyTo(a,r); return r; }\r\n\r\n// (public) this / a\r\nfunction bnDivide(a) { var r = nbi(); this.divRemTo(a,r,null); return r; }\r\n\r\n// (public) this % a\r\nfunction bnRemainder(a) { var r = nbi(); this.divRemTo(a,null,r); return r; }\r\n\r\n// (public) [this/a,this%a]\r\nfunction bnDivideAndRemainder(a) {\r\n  var q = nbi(), r = nbi();\r\n  this.divRemTo(a,q,r);\r\n  return new Array(q,r);\r\n}\r\n\r\n// (protected) this *= n, this >= 0, 1 < n < DV\r\nfunction bnpDMultiply(n) {\r\n  var this_array = this.array;\r\n  this_array[this.t] = this.am(0,n-1,this,0,0,this.t);\r\n  ++this.t;\r\n  this.clamp();\r\n}\r\n\r\n// (protected) this += n << w words, this >= 0\r\nfunction bnpDAddOffset(n,w) {\r\n  var this_array = this.array;\r\n  while(this.t <= w) this_array[this.t++] = 0;\r\n  this_array[w] += n;\r\n  while(this_array[w] >= BI_DV) {\r\n    this_array[w] -= BI_DV;\r\n    if(++w >= this.t) this_array[this.t++] = 0;\r\n    ++this_array[w];\r\n  }\r\n}\r\n\r\n// A \"null\" reducer\r\nfunction NullExp() {}\r\nfunction nNop(x) { return x; }\r\nfunction nMulTo(x,y,r) { x.multiplyTo(y,r); }\r\nfunction nSqrTo(x,r) { x.squareTo(r); }\r\n\r\nNullExp.prototype.convert = nNop;\r\nNullExp.prototype.revert = nNop;\r\nNullExp.prototype.mulTo = nMulTo;\r\nNullExp.prototype.sqrTo = nSqrTo;\r\n\r\n// (public) this^e\r\nfunction bnPow(e) { return this.exp(e,new NullExp()); }\r\n\r\n// (protected) r = lower n words of \"this * a\", a.t <= n\r\n// \"this\" should be the larger one if appropriate.\r\nfunction bnpMultiplyLowerTo(a,n,r) {\r\n  var r_array = r.array;\r\n  var a_array = a.array;\r\n  var i = Math.min(this.t+a.t,n);\r\n  r.s = 0; // assumes a,this >= 0\r\n  r.t = i;\r\n  while(i > 0) r_array[--i] = 0;\r\n  var j;\r\n  for(j = r.t-this.t; i < j; ++i) r_array[i+this.t] = this.am(0,a_array[i],r,i,0,this.t);\r\n  for(j = Math.min(a.t,n); i < j; ++i) this.am(0,a_array[i],r,i,0,n-i);\r\n  r.clamp();\r\n}\r\n\r\n// (protected) r = \"this * a\" without lower n words, n > 0\r\n// \"this\" should be the larger one if appropriate.\r\nfunction bnpMultiplyUpperTo(a,n,r) {\r\n  var r_array = r.array;\r\n  var a_array = a.array;\r\n  --n;\r\n  var i = r.t = this.t+a.t-n;\r\n  r.s = 0; // assumes a,this >= 0\r\n  while(--i >= 0) r_array[i] = 0;\r\n  for(i = Math.max(n-this.t,0); i < a.t; ++i)\r\n    r_array[this.t+i-n] = this.am(n-i,a_array[i],r,0,0,this.t+i-n);\r\n  r.clamp();\r\n  r.drShiftTo(1,r);\r\n}\r\n\r\n// Barrett modular reduction\r\nfunction Barrett(m) {\r\n  // setup Barrett\r\n  this.r2 = nbi();\r\n  this.q3 = nbi();\r\n  BigInteger.ONE.dlShiftTo(2*m.t,this.r2);\r\n  this.mu = this.r2.divide(m);\r\n  this.m = m;\r\n}\r\n\r\nfunction barrettConvert(x) {\r\n  if(x.s < 0 || x.t > 2*this.m.t) return x.mod(this.m);\r\n  else if(x.compareTo(this.m) < 0) return x;\r\n  else { var r = nbi(); x.copyTo(r); this.reduce(r); return r; }\r\n}\r\n\r\nfunction barrettRevert(x) { return x; }\r\n\r\n// x = x mod m (HAC 14.42)\r\nfunction barrettReduce(x) {\r\n  x.drShiftTo(this.m.t-1,this.r2);\r\n  if(x.t > this.m.t+1) { x.t = this.m.t+1; x.clamp(); }\r\n  this.mu.multiplyUpperTo(this.r2,this.m.t+1,this.q3);\r\n  this.m.multiplyLowerTo(this.q3,this.m.t+1,this.r2);\r\n  while(x.compareTo(this.r2) < 0) x.dAddOffset(1,this.m.t+1);\r\n  x.subTo(this.r2,x);\r\n  while(x.compareTo(this.m) >= 0) x.subTo(this.m,x);\r\n}\r\n\r\n// r = x^2 mod m; x != r\r\nfunction barrettSqrTo(x,r) { x.squareTo(r); this.reduce(r); }\r\n\r\n// r = x*y mod m; x,y != r\r\nfunction barrettMulTo(x,y,r) { x.multiplyTo(y,r); this.reduce(r); }\r\n\r\nBarrett.prototype.convert = barrettConvert;\r\nBarrett.prototype.revert = barrettRevert;\r\nBarrett.prototype.reduce = barrettReduce;\r\nBarrett.prototype.mulTo = barrettMulTo;\r\nBarrett.prototype.sqrTo = barrettSqrTo;\r\n\r\n// (public) this^e % m (HAC 14.85)\r\nfunction bnModPow(e,m) {\r\n  var e_array = e.array;\r\n  var i = e.bitLength(), k, r = nbv(1), z;\r\n  if(i <= 0) return r;\r\n  else if(i < 18) k = 1;\r\n  else if(i < 48) k = 3;\r\n  else if(i < 144) k = 4;\r\n  else if(i < 768) k = 5;\r\n  else k = 6;\r\n  if(i < 8)\r\n    z = new Classic(m);\r\n  else if(m.isEven())\r\n    z = new Barrett(m);\r\n  else\r\n    z = new Montgomery(m);\r\n\r\n  // precomputation\r\n  var g = new Array(), n = 3, k1 = k-1, km = (1<<k)-1;\r\n  g[1] = z.convert(this);\r\n  if(k > 1) {\r\n    var g2 = nbi();\r\n    z.sqrTo(g[1],g2);\r\n    while(n <= km) {\r\n      g[n] = nbi();\r\n      z.mulTo(g2,g[n-2],g[n]);\r\n      n += 2;\r\n    }\r\n  }\r\n\r\n  var j = e.t-1, w, is1 = true, r2 = nbi(), t;\r\n  i = nbits(e_array[j])-1;\r\n  while(j >= 0) {\r\n    if(i >= k1) w = (e_array[j]>>(i-k1))&km;\r\n    else {\r\n      w = (e_array[j]&((1<<(i+1))-1))<<(k1-i);\r\n      if(j > 0) w |= e_array[j-1]>>(BI_DB+i-k1);\r\n    }\r\n\r\n    n = k;\r\n    while((w&1) == 0) { w >>= 1; --n; }\r\n    if((i -= n) < 0) { i += BI_DB; --j; }\r\n    if(is1) {\t// ret == 1, don't bother squaring or multiplying it\r\n      g[w].copyTo(r);\r\n      is1 = false;\r\n    }\r\n    else {\r\n      while(n > 1) { z.sqrTo(r,r2); z.sqrTo(r2,r); n -= 2; }\r\n      if(n > 0) z.sqrTo(r,r2); else { t = r; r = r2; r2 = t; }\r\n      z.mulTo(r2,g[w],r);\r\n    }\r\n\r\n    while(j >= 0 && (e_array[j]&(1<<i)) == 0) {\r\n      z.sqrTo(r,r2); t = r; r = r2; r2 = t;\r\n      if(--i < 0) { i = BI_DB-1; --j; }\r\n    }\r\n  }\r\n  return z.revert(r);\r\n}\r\n\r\n// (public) gcd(this,a) (HAC 14.54)\r\nfunction bnGCD(a) {\r\n  var x = (this.s<0)?this.negate():this.clone();\r\n  var y = (a.s<0)?a.negate():a.clone();\r\n  if(x.compareTo(y) < 0) { var t = x; x = y; y = t; }\r\n  var i = x.getLowestSetBit(), g = y.getLowestSetBit();\r\n  if(g < 0) return x;\r\n  if(i < g) g = i;\r\n  if(g > 0) {\r\n    x.rShiftTo(g,x);\r\n    y.rShiftTo(g,y);\r\n  }\r\n  while(x.signum() > 0) {\r\n    if((i = x.getLowestSetBit()) > 0) x.rShiftTo(i,x);\r\n    if((i = y.getLowestSetBit()) > 0) y.rShiftTo(i,y);\r\n    if(x.compareTo(y) >= 0) {\r\n      x.subTo(y,x);\r\n      x.rShiftTo(1,x);\r\n    }\r\n    else {\r\n      y.subTo(x,y);\r\n      y.rShiftTo(1,y);\r\n    }\r\n  }\r\n  if(g > 0) y.lShiftTo(g,y);\r\n  return y;\r\n}\r\n\r\n// (protected) this % n, n < 2^26\r\nfunction bnpModInt(n) {\r\n  var this_array = this.array;\r\n  if(n <= 0) return 0;\r\n  var d = BI_DV%n, r = (this.s<0)?n-1:0;\r\n  if(this.t > 0)\r\n    if(d == 0) r = this_array[0]%n;\r\n    else for(var i = this.t-1; i >= 0; --i) r = (d*r+this_array[i])%n;\r\n  return r;\r\n}\r\n\r\n// (public) 1/this % m (HAC 14.61)\r\nfunction bnModInverse(m) {\r\n  var ac = m.isEven();\r\n  if((this.isEven() && ac) || m.signum() == 0) return BigInteger.ZERO;\r\n  var u = m.clone(), v = this.clone();\r\n  var a = nbv(1), b = nbv(0), c = nbv(0), d = nbv(1);\r\n  while(u.signum() != 0) {\r\n    while(u.isEven()) {\r\n      u.rShiftTo(1,u);\r\n      if(ac) {\r\n        if(!a.isEven() || !b.isEven()) { a.addTo(this,a); b.subTo(m,b); }\r\n        a.rShiftTo(1,a);\r\n      }\r\n      else if(!b.isEven()) b.subTo(m,b);\r\n      b.rShiftTo(1,b);\r\n    }\r\n    while(v.isEven()) {\r\n      v.rShiftTo(1,v);\r\n      if(ac) {\r\n        if(!c.isEven() || !d.isEven()) { c.addTo(this,c); d.subTo(m,d); }\r\n        c.rShiftTo(1,c);\r\n      }\r\n      else if(!d.isEven()) d.subTo(m,d);\r\n      d.rShiftTo(1,d);\r\n    }\r\n    if(u.compareTo(v) >= 0) {\r\n      u.subTo(v,u);\r\n      if(ac) a.subTo(c,a);\r\n      b.subTo(d,b);\r\n    }\r\n    else {\r\n      v.subTo(u,v);\r\n      if(ac) c.subTo(a,c);\r\n      d.subTo(b,d);\r\n    }\r\n  }\r\n  if(v.compareTo(BigInteger.ONE) != 0) return BigInteger.ZERO;\r\n  if(d.compareTo(m) >= 0) return d.subtract(m);\r\n  if(d.signum() < 0) d.addTo(m,d); else return d;\r\n  if(d.signum() < 0) return d.add(m); else return d;\r\n}\r\n\r\nvar lowprimes = [2,3,5,7,11,13,17,19,23,29,31,37,41,43,47,53,59,61,67,71,73,79,83,89,97,101,103,107,109,113,127,131,137,139,149,151,157,163,167,173,179,181,191,193,197,199,211,223,227,229,233,239,241,251,257,263,269,271,277,281,283,293,307,311,313,317,331,337,347,349,353,359,367,373,379,383,389,397,401,409,419,421,431,433,439,443,449,457,461,463,467,479,487,491,499,503,509];\r\nvar lplim = (1<<26)/lowprimes[lowprimes.length-1];\r\n\r\n// (public) test primality with certainty >= 1-.5^t\r\nfunction bnIsProbablePrime(t) {\r\n  var i, x = this.abs();\r\n  var x_array = x.array;\r\n  if(x.t == 1 && x_array[0] <= lowprimes[lowprimes.length-1]) {\r\n    for(i = 0; i < lowprimes.length; ++i)\r\n      if(x_array[0] == lowprimes[i]) return true;\r\n    return false;\r\n  }\r\n  if(x.isEven()) return false;\r\n  i = 1;\r\n  while(i < lowprimes.length) {\r\n    var m = lowprimes[i], j = i+1;\r\n    while(j < lowprimes.length && m < lplim) m *= lowprimes[j++];\r\n    m = x.modInt(m);\r\n    while(i < j) if(m%lowprimes[i++] == 0) return false;\r\n  }\r\n  return x.millerRabin(t);\r\n}\r\n\r\n// (protected) true if probably prime (HAC 4.24, Miller-Rabin)\r\nfunction bnpMillerRabin(t) {\r\n  var n1 = this.subtract(BigInteger.ONE);\r\n  var k = n1.getLowestSetBit();\r\n  if(k <= 0) return false;\r\n  var r = n1.shiftRight(k);\r\n  t = (t+1)>>1;\r\n  if(t > lowprimes.length) t = lowprimes.length;\r\n  var a = nbi();\r\n  for(var i = 0; i < t; ++i) {\r\n    a.fromInt(lowprimes[i]);\r\n    var y = a.modPow(r,this);\r\n    if(y.compareTo(BigInteger.ONE) != 0 && y.compareTo(n1) != 0) {\r\n      var j = 1;\r\n      while(j++ < k && y.compareTo(n1) != 0) {\r\n        y = y.modPowInt(2,this);\r\n        if(y.compareTo(BigInteger.ONE) == 0) return false;\r\n      }\r\n      if(y.compareTo(n1) != 0) return false;\r\n    }\r\n  }\r\n  return true;\r\n}\r\n\r\n// protected\r\nBigInteger.prototype.chunkSize = bnpChunkSize;\r\nBigInteger.prototype.toRadix = bnpToRadix;\r\nBigInteger.prototype.fromRadix = bnpFromRadix;\r\nBigInteger.prototype.fromNumber = bnpFromNumber;\r\nBigInteger.prototype.bitwiseTo = bnpBitwiseTo;\r\nBigInteger.prototype.changeBit = bnpChangeBit;\r\nBigInteger.prototype.addTo = bnpAddTo;\r\nBigInteger.prototype.dMultiply = bnpDMultiply;\r\nBigInteger.prototype.dAddOffset = bnpDAddOffset;\r\nBigInteger.prototype.multiplyLowerTo = bnpMultiplyLowerTo;\r\nBigInteger.prototype.multiplyUpperTo = bnpMultiplyUpperTo;\r\nBigInteger.prototype.modInt = bnpModInt;\r\nBigInteger.prototype.millerRabin = bnpMillerRabin;\r\n\r\n// public\r\nBigInteger.prototype.clone = bnClone;\r\nBigInteger.prototype.intValue = bnIntValue;\r\nBigInteger.prototype.byteValue = bnByteValue;\r\nBigInteger.prototype.shortValue = bnShortValue;\r\nBigInteger.prototype.signum = bnSigNum;\r\nBigInteger.prototype.toByteArray = bnToByteArray;\r\nBigInteger.prototype.equals = bnEquals;\r\nBigInteger.prototype.min = bnMin;\r\nBigInteger.prototype.max = bnMax;\r\nBigInteger.prototype.and = bnAnd;\r\nBigInteger.prototype.or = bnOr;\r\nBigInteger.prototype.xor = bnXor;\r\nBigInteger.prototype.andNot = bnAndNot;\r\nBigInteger.prototype.not = bnNot;\r\nBigInteger.prototype.shiftLeft = bnShiftLeft;\r\nBigInteger.prototype.shiftRight = bnShiftRight;\r\nBigInteger.prototype.getLowestSetBit = bnGetLowestSetBit;\r\nBigInteger.prototype.bitCount = bnBitCount;\r\nBigInteger.prototype.testBit = bnTestBit;\r\nBigInteger.prototype.setBit = bnSetBit;\r\nBigInteger.prototype.clearBit = bnClearBit;\r\nBigInteger.prototype.flipBit = bnFlipBit;\r\nBigInteger.prototype.add = bnAdd;\r\nBigInteger.prototype.subtract = bnSubtract;\r\nBigInteger.prototype.multiply = bnMultiply;\r\nBigInteger.prototype.divide = bnDivide;\r\nBigInteger.prototype.remainder = bnRemainder;\r\nBigInteger.prototype.divideAndRemainder = bnDivideAndRemainder;\r\nBigInteger.prototype.modPow = bnModPow;\r\nBigInteger.prototype.modInverse = bnModInverse;\r\nBigInteger.prototype.pow = bnPow;\r\nBigInteger.prototype.gcd = bnGCD;\r\nBigInteger.prototype.isProbablePrime = bnIsProbablePrime;\r\n\r\n// BigInteger interfaces not implemented in jsbn:\r\n\r\n// BigInteger(int signum, byte[] magnitude)\r\n// double doubleValue()\r\n// float floatValue()\r\n// int hashCode()\r\n// long longValue()\r\n// static BigInteger valueOf(long val)\r\n// prng4.js - uses Arcfour as a PRNG\r\n\r\nfunction Arcfour() {\r\n  this.i = 0;\r\n  this.j = 0;\r\n  this.S = new Array();\r\n}\r\n\r\n// Initialize arcfour context from key, an array of ints, each from [0..255]\r\nfunction ARC4init(key) {\r\n  var i, j, t;\r\n  for(i = 0; i < 256; ++i)\r\n    this.S[i] = i;\r\n  j = 0;\r\n  for(i = 0; i < 256; ++i) {\r\n    j = (j + this.S[i] + key[i % key.length]) & 255;\r\n    t = this.S[i];\r\n    this.S[i] = this.S[j];\r\n    this.S[j] = t;\r\n  }\r\n  this.i = 0;\r\n  this.j = 0;\r\n}\r\n\r\nfunction ARC4next() {\r\n  var t;\r\n  this.i = (this.i + 1) & 255;\r\n  this.j = (this.j + this.S[this.i]) & 255;\r\n  t = this.S[this.i];\r\n  this.S[this.i] = this.S[this.j];\r\n  this.S[this.j] = t;\r\n  return this.S[(t + this.S[this.i]) & 255];\r\n}\r\n\r\nArcfour.prototype.init = ARC4init;\r\nArcfour.prototype.next = ARC4next;\r\n\r\n// Plug in your RNG constructor here\r\nfunction prng_newstate() {\r\n  return new Arcfour();\r\n}\r\n\r\n// Pool size must be a multiple of 4 and greater than 32.\r\n// An array of bytes the size of the pool will be passed to init()\r\nvar rng_psize = 256;\r\n// Random number generator - requires a PRNG backend, e.g. prng4.js\r\n\r\n// For best results, put code like\r\n// <body onClick='rng_seed_time();' onKeyPress='rng_seed_time();'>\r\n// in your main HTML document.\r\n\r\nvar rng_state;\r\nvar rng_pool;\r\nvar rng_pptr;\r\n\r\n// Mix in a 32-bit integer into the pool\r\nfunction rng_seed_int(x) {\r\n  rng_pool[rng_pptr++] ^= x & 255;\r\n  rng_pool[rng_pptr++] ^= (x >> 8) & 255;\r\n  rng_pool[rng_pptr++] ^= (x >> 16) & 255;\r\n  rng_pool[rng_pptr++] ^= (x >> 24) & 255;\r\n  if(rng_pptr >= rng_psize) rng_pptr -= rng_psize;\r\n}\r\n\r\n// Mix in the current time (w/milliseconds) into the pool\r\nfunction rng_seed_time() {\r\n  // Use pre-computed date to avoid making the benchmark\r\n  // results dependent on the current date.\r\n  rng_seed_int(1122926989487);\r\n}\r\n\r\n// Initialize the pool with junk if needed.\r\nif(rng_pool == null) {\r\n  rng_pool = new Array();\r\n  rng_pptr = 0;\r\n  var t;\r\n  while(rng_pptr < rng_psize) {  // extract some randomness from Math.random()\r\n    t = Math.floor(65536 * Math.random());\r\n    rng_pool[rng_pptr++] = t >>> 8;\r\n    rng_pool[rng_pptr++] = t & 255;\r\n  }\r\n  rng_pptr = 0;\r\n  rng_seed_time();\r\n  //rng_seed_int(window.screenX);\r\n  //rng_seed_int(window.screenY);\r\n}\r\n\r\nfunction rng_get_byte() {\r\n  if(rng_state == null) {\r\n    rng_seed_time();\r\n    rng_state = prng_newstate();\r\n    rng_state.init(rng_pool);\r\n    for(rng_pptr = 0; rng_pptr < rng_pool.length; ++rng_pptr)\r\n      rng_pool[rng_pptr] = 0;\r\n    rng_pptr = 0;\r\n    //rng_pool = null;\r\n  }\r\n  // TODO: allow reseeding after first request\r\n  return rng_state.next();\r\n}\r\n\r\nfunction rng_get_bytes(ba) {\r\n  var i;\r\n  for(i = 0; i < ba.length; ++i) ba[i] = rng_get_byte();\r\n}\r\n\r\nfunction SecureRandom() {}\r\n\r\nSecureRandom.prototype.nextBytes = rng_get_bytes;\r\n// Depends on jsbn.js and rng.js\r\n\r\n// convert a (hex) string to a bignum object\r\nfunction parseBigInt(str,r) {\r\n  return new BigInteger(str,r);\r\n}\r\n\r\nfunction linebrk(s,n) {\r\n  var ret = \"\";\r\n  var i = 0;\r\n  while(i + n < s.length) {\r\n    ret += s.substring(i,i+n) + \"\\n\";\r\n    i += n;\r\n  }\r\n  return ret + s.substring(i,s.length);\r\n}\r\n\r\nfunction byte2Hex(b) {\r\n  if(b < 0x10)\r\n    return \"0\" + b.toString(16);\r\n  else\r\n    return b.toString(16);\r\n}\r\n\r\n// PKCS#1 (type 2, random) pad input string s to n bytes, and return a bigint\r\nfunction pkcs1pad2(s,n) {\r\n  if(n < s.length + 11) {\r\n    alert(\"Message too long for RSA\");\r\n    return null;\r\n  }\r\n  var ba = new Array();\r\n  var i = s.length - 1;\r\n  while(i >= 0 && n > 0) ba[--n] = s.charCodeAt(i--);\r\n  ba[--n] = 0;\r\n  var rng = new SecureRandom();\r\n  var x = new Array();\r\n  while(n > 2) { // random non-zero pad\r\n    x[0] = 0;\r\n    while(x[0] == 0) rng.nextBytes(x);\r\n    ba[--n] = x[0];\r\n  }\r\n  ba[--n] = 2;\r\n  ba[--n] = 0;\r\n  return new BigInteger(ba);\r\n}\r\n\r\n// \"empty\" RSA key constructor\r\nfunction RSAKey() {\r\n  this.n = null;\r\n  this.e = 0;\r\n  this.d = null;\r\n  this.p = null;\r\n  this.q = null;\r\n  this.dmp1 = null;\r\n  this.dmq1 = null;\r\n  this.coeff = null;\r\n}\r\n\r\n// Set the public key fields N and e from hex strings\r\nfunction RSASetPublic(N,E) {\r\n  if(N != null && E != null && N.length > 0 && E.length > 0) {\r\n    this.n = parseBigInt(N,16);\r\n    this.e = parseInt(E,16);\r\n  }\r\n  else\r\n    alert(\"Invalid RSA public key\");\r\n}\r\n\r\n// Perform raw public operation on \"x\": return x^e (mod n)\r\nfunction RSADoPublic(x) {\r\n  return x.modPowInt(this.e, this.n);\r\n}\r\n\r\n// Return the PKCS#1 RSA encryption of \"text\" as an even-length hex string\r\nfunction RSAEncrypt(text) {\r\n  var m = pkcs1pad2(text,(this.n.bitLength()+7)>>3);\r\n  if(m == null) return null;\r\n  var c = this.doPublic(m);\r\n  if(c == null) return null;\r\n  var h = c.toString(16);\r\n  if((h.length & 1) == 0) return h; else return \"0\" + h;\r\n}\r\n\r\n// Return the PKCS#1 RSA encryption of \"text\" as a Base64-encoded string\r\n//function RSAEncryptB64(text) {\r\n//  var h = this.encrypt(text);\r\n//  if(h) return hex2b64(h); else return null;\r\n//}\r\n\r\n// protected\r\nRSAKey.prototype.doPublic = RSADoPublic;\r\n\r\n// public\r\nRSAKey.prototype.setPublic = RSASetPublic;\r\nRSAKey.prototype.encrypt = RSAEncrypt;\r\n//RSAKey.prototype.encrypt_b64 = RSAEncryptB64;\r\n// Depends on rsa.js and jsbn2.js\r\n\r\n// Undo PKCS#1 (type 2, random) padding and, if valid, return the plaintext\r\nfunction pkcs1unpad2(d,n) {\r\n  var b = d.toByteArray();\r\n  var i = 0;\r\n  while(i < b.length && b[i] == 0) ++i;\r\n  if(b.length-i != n-1 || b[i] != 2)\r\n    return null;\r\n  ++i;\r\n  while(b[i] != 0)\r\n    if(++i >= b.length) return null;\r\n  var ret = \"\";\r\n  while(++i < b.length)\r\n    ret += String.fromCharCode(b[i]);\r\n  return ret;\r\n}\r\n\r\n// Set the private key fields N, e, and d from hex strings\r\nfunction RSASetPrivate(N,E,D) {\r\n  if(N != null && E != null && N.length > 0 && E.length > 0) {\r\n    this.n = parseBigInt(N,16);\r\n    this.e = parseInt(E,16);\r\n    this.d = parseBigInt(D,16);\r\n  }\r\n  else\r\n    alert(\"Invalid RSA private key\");\r\n}\r\n\r\n// Set the private key fields N, e, d and CRT params from hex strings\r\nfunction RSASetPrivateEx(N,E,D,P,Q,DP,DQ,C) {\r\n  if(N != null && E != null && N.length > 0 && E.length > 0) {\r\n    this.n = parseBigInt(N,16);\r\n    this.e = parseInt(E,16);\r\n    this.d = parseBigInt(D,16);\r\n    this.p = parseBigInt(P,16);\r\n    this.q = parseBigInt(Q,16);\r\n    this.dmp1 = parseBigInt(DP,16);\r\n    this.dmq1 = parseBigInt(DQ,16);\r\n    this.coeff = parseBigInt(C,16);\r\n  }\r\n  else\r\n    alert(\"Invalid RSA private key\");\r\n}\r\n\r\n// Generate a new random private key B bits long, using public expt E\r\nfunction RSAGenerate(B,E) {\r\n  var rng = new SecureRandom();\r\n  var qs = B>>1;\r\n  this.e = parseInt(E,16);\r\n  var ee = new BigInteger(E,16);\r\n  for(;;) {\r\n    for(;;) {\r\n      this.p = new BigInteger(B-qs,1,rng);\r\n      if(this.p.subtract(BigInteger.ONE).gcd(ee).compareTo(BigInteger.ONE) == 0 && this.p.isProbablePrime(10)) break;\r\n    }\r\n    for(;;) {\r\n      this.q = new BigInteger(qs,1,rng);\r\n      if(this.q.subtract(BigInteger.ONE).gcd(ee).compareTo(BigInteger.ONE) == 0 && this.q.isProbablePrime(10)) break;\r\n    }\r\n    if(this.p.compareTo(this.q) <= 0) {\r\n      var t = this.p;\r\n      this.p = this.q;\r\n      this.q = t;\r\n    }\r\n    var p1 = this.p.subtract(BigInteger.ONE);\r\n    var q1 = this.q.subtract(BigInteger.ONE);\r\n    var phi = p1.multiply(q1);\r\n    if(phi.gcd(ee).compareTo(BigInteger.ONE) == 0) {\r\n      this.n = this.p.multiply(this.q);\r\n      this.d = ee.modInverse(phi);\r\n      this.dmp1 = this.d.mod(p1);\r\n      this.dmq1 = this.d.mod(q1);\r\n      this.coeff = this.q.modInverse(this.p);\r\n      break;\r\n    }\r\n  }\r\n}\r\n\r\n// Perform raw private operation on \"x\": return x^d (mod n)\r\nfunction RSADoPrivate(x) {\r\n  if(this.p == null || this.q == null)\r\n    return x.modPow(this.d, this.n);\r\n\r\n  // TODO: re-calculate any missing CRT params\r\n  var xp = x.mod(this.p).modPow(this.dmp1, this.p);\r\n  var xq = x.mod(this.q).modPow(this.dmq1, this.q);\r\n\r\n  while(xp.compareTo(xq) < 0)\r\n    xp = xp.add(this.p);\r\n  return xp.subtract(xq).multiply(this.coeff).mod(this.p).multiply(this.q).add(xq);\r\n}\r\n\r\n// Return the PKCS#1 RSA decryption of \"ctext\".\r\n// \"ctext\" is an even-length hex string and the output is a plain string.\r\nfunction RSADecrypt(ctext) {\r\n  var c = parseBigInt(ctext, 16);\r\n  var m = this.doPrivate(c);\r\n  if(m == null) return null;\r\n  return pkcs1unpad2(m, (this.n.bitLength()+7)>>3);\r\n}\r\n\r\n// Return the PKCS#1 RSA decryption of \"ctext\".\r\n// \"ctext\" is a Base64-encoded string and the output is a plain string.\r\n//function RSAB64Decrypt(ctext) {\r\n//  var h = b64tohex(ctext);\r\n//  if(h) return this.decrypt(h); else return null;\r\n//}\r\n\r\n// protected\r\nRSAKey.prototype.doPrivate = RSADoPrivate;\r\n\r\n// public\r\nRSAKey.prototype.setPrivate = RSASetPrivate;\r\nRSAKey.prototype.setPrivateEx = RSASetPrivateEx;\r\nRSAKey.prototype.generate = RSAGenerate;\r\nRSAKey.prototype.decrypt = RSADecrypt;\r\n//RSAKey.prototype.b64_decrypt = RSAB64Decrypt;\r\n\r\n\r\nnValue=\"a5261939975948bb7a58dffe5ff54e65f0498f9175f5a09288810b8975871e99af3b5dd94057b0fc07535f5f97444504fa35169d461d0d30cf0192e307727c065168c788771c561a9400fb49175e9e6aa4e23fe11af69e9412dd23b0cb6684c4c2429bce139e848ab26d0829073351f4acd36074eafd036a5eb83359d2a698d3\";\r\neValue=\"10001\";\r\ndValue=\"8e9912f6d3645894e8d38cb58c0db81ff516cf4c7e5a14c7f1eddb1459d2cded4d8d293fc97aee6aefb861859c8b6a3d1dfe710463e1f9ddc72048c09751971c4a580aa51eb523357a3cc48d31cfad1d4a165066ed92d4748fb6571211da5cb14bc11b6e2df7c1a559e6d5ac1cd5c94703a22891464fba23d0d965086277a161\";\r\npValue=\"d090ce58a92c75233a6486cb0a9209bf3583b64f540c76f5294bb97d285eed33aec220bde14b2417951178ac152ceab6da7090905b478195498b352048f15e7d\";\r\nqValue=\"cab575dc652bb66df15a0359609d51d1db184750c00c6698b90ef3465c99655103edbf0d54c56aec0ce3c4d22592338092a126a0cc49f65a4a30d222b411e58f\";\r\ndmp1Value=\"1a24bca8e273df2f0e47c199bbf678604e7df7215480c77c8db39f49b000ce2cf7500038acfff5433b7d582a01f1826e6f4d42e1c57f5e1fef7b12aabc59fd25\";\r\ndmq1Value=\"3d06982efbbe47339e1f6d36b1216b8a741d410b0c662f54f7118b27b9a4ec9d914337eb39841d8666f3034408cf94f5b62f11c402fc994fe15a05493150d9fd\";\r\ncoeffValue=\"3a3e731acd8960b7ff9eb81a7ff93bd1cfa74cbd56987db58b4594fb09c09084db1734c8143f98b602b981aaa9243ca28deb69b5b280ee8dcee0fd2625e53250\";\r\n\r\nsetupEngine(am3, 28);\r\n\r\nvar TEXT = \"The quick brown fox jumped over the extremely lazy frog! \" +\r\n    \"Now is the time for all good men to come to the party.\";\r\nvar encrypted;\r\n\r\nfunction encrypt() {\r\n  var RSA = new RSAKey();\r\n  RSA.setPublic(nValue, eValue);\r\n  RSA.setPrivateEx(nValue, eValue, dValue, pValue, qValue, dmp1Value, dmq1Value, coeffValue);\r\n  encrypted = RSA.encrypt(TEXT);\r\n}\r\n\r\nfunction decrypt() {\r\n  var RSA = new RSAKey();\r\n  RSA.setPublic(nValue, eValue);\r\n  RSA.setPrivateEx(nValue, eValue, dValue, pValue, qValue, dmp1Value, dmq1Value, coeffValue);\r\n  var decrypted = RSA.decrypt(encrypted);\r\n  if (decrypted != TEXT) {\r\n    throw new Error(\"Crypto operation failed\");\r\n  }\r\n}\r\n\r\nclass Benchmark {\r\n    runIteration() {\r\n        encrypt();\r\n        decrypt();\r\n    }\r\n}\r\n\r\n"};
var readFile = function (name) {
    const normalized = String(name).replaceAll("\\", "/");
    if (!Object.prototype.hasOwnProperty.call(__jetstreamResources, normalized))
        throw new Error("JetStream resource not embedded: " + normalized);
    return __jetstreamResources[normalized];
};

"use strict";

/*
 * Copyright (C) 2018 Apple Inc. All rights reserved.
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

const preloadResources = !isInBrowser;
const measureTotalTimeAsSubtest = false; // Once we move to preloading all resources, it would be good to turn this on.

if (typeof RAMification === "undefined")
    var RAMification = false;

if (typeof testIterationCount === "undefined")
    var testIterationCount = undefined;

// Used for the promise representing the current benchmark run.
var currentResolve = null;
var currentReject = null;

const defaultIterationCount = 120;
const defaultWorstCaseCount = 4;

function assert(b, m = "") {
    if (!b)
        throw new Error("Bad assertion: " + m);
}

function firstID(benchmark) {
    return `results-cell-${benchmark.name}-first`;
}

function worst4ID(benchmark) {
    return `results-cell-${benchmark.name}-worst4`;
}

function avgID(benchmark) {
    return `results-cell-${benchmark.name}-avg`;
}

function scoreID(benchmark) {
    return `results-cell-${benchmark.name}-score`;
}

function mean(values) {
    assert(values instanceof Array);
    let sum = 0;
    for (let x of values)
        sum += x;
    return sum / values.length;
}

function geomean(values) {
    assert(values instanceof Array);
    let product = 1;
    for (let x of values)
        product *= x;
    return product ** (1 / values.length);
}

function toScore(timeValue) {
    return 5000 / timeValue;
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
    return num.toFixed(3);
}

function uiFriendlyDuration(time)
{
    let minutes = time.getMinutes();
    let seconds = time.getSeconds();
    let milliSeconds = time.getMilliseconds();
    let result = "" + minutes + ":";

    result = result + (seconds < 10 ? "0" : "") + seconds + ".";
    result = result + (milliSeconds < 10 ? "00" : (milliSeconds < 100 ? "0" : "")) + milliSeconds;

    return result;
}

const fileLoader = (function() {
    class Loader {
        constructor() {
            this.requests = new Map;
        }

        async _loadInternal(url) {
            if (!isInBrowser)
                return Promise.resolve(readFile(url));

            let fetchResponse = await fetch(new Request(url));
            if (url.indexOf(".js") !== -1)
                return await fetchResponse.text();
            else if (url.indexOf(".wasm") !== -1)
                return await fetchResponse.arrayBuffer();

            throw new Error("should not be reached!");
        }

        async load(url) {
            if (this.requests.has(url))
                return (await this.requests.get(url));

            let promise = this._loadInternal(url);
            this.requests.set(url, promise);
            return (await promise);
        }
    }
    return new Loader;
})();

class Driver {
    constructor() {
        this.benchmarks = [];
    }

    addPlan(plan, BenchmarkClass = DefaultBenchmark) {
        this.benchmarks.push(new BenchmarkClass(plan));
    }

    async start() {
        let statusElement = false;
        let summaryElement = false;
        if (isInBrowser) {
            statusElement = document.getElementById("status");
            summaryElement = document.getElementById("result-summary");
            statusElement.innerHTML = `<label>Running...</label>`;
        } else {
            console.log("Starting JetStream2");
        }

        await updateUI();

        let __jetstreamSuiteStart = Date.now();
        for (let benchmark of this.benchmarks) {
            benchmark.updateUIBeforeRun();

            await updateUI();

            try {

                await benchmark.run();
            } catch(e) {
                JetStream.reportError(benchmark);
                throw e;
            }

            benchmark.updateUIAfterRun();
        }

        let totalTime = Date.now() - __jetstreamSuiteStart;
        if (measureTotalTimeAsSubtest) {
            if (isInBrowser)
                document.getElementById("benchmark-total-time-score").innerHTML = uiFriendlyNumber(totalTime);
            else
                console.log("Total time:", uiFriendlyNumber(totalTime));
            allScores.push(totalTime);
        }

        let allScores = [];
        for (let benchmark of this.benchmarks)
            allScores.push(benchmark.score);

        if (isInBrowser) {
            summaryElement.classList.add('done');
            summaryElement.innerHTML = "<div class=\"score\">" + uiFriendlyNumber(geomean(allScores)) + "</div><label>Score</label>";
            statusElement.innerHTML = '';
        } else
            console.log("\nTotal Score: ", uiFriendlyNumber(geomean(allScores)), "\n");

        this.reportScoreToRunBenchmarkRunner();
    }

    runCode(string)
    {
        if (!isInBrowser) {
            let top = { currentResolve, currentReject };
            new Function("top", string.join("\n"))(top);
            return globalThis;
        }

        var magic = document.getElementById("magic");
        magic.contentDocument.body.textContent = "";
        magic.contentDocument.body.innerHTML = "<iframe id=\"magicframe\" frameborder=\"0\">";

        var magicFrame = magic.contentDocument.getElementById("magicframe");
        magicFrame.contentDocument.open();
        magicFrame.contentDocument.write("<!DOCTYPE html><head><title>benchmark payload</title></head><body>\n" + string + "</body></html>");

        return magicFrame;
    }

    prepareToRun()
    {
        this.benchmarks.sort((a, b) => a.plan.name.toLowerCase() < b.plan.name.toLowerCase() ? 1 : -1);

        let text = "";
        let newBenchmarks = [];
        for (let benchmark of this.benchmarks) {
            let id = JSON.stringify(benchmark.constructor.scoreDescription());
            let description = JSON.parse(id);

            newBenchmarks.push(benchmark);
            let scoreIds = benchmark.scoreIdentifiers()
            let overallScoreId = scoreIds.pop();

            if (isInBrowser) {
                text +=
                    `<div class="benchmark" id="benchmark-${benchmark.name}">
                    <h3 class="benchmark-name"><a href="in-depth.html#${benchmark.name}">${benchmark.name}</a></h3>
                    <h4 class="score" id="${overallScoreId}">___</h4><p>`;
                for (let i = 0; i < scoreIds.length; i++) {
                    let id = scoreIds[i];
                    let label = description[i];
                    text += `<span class="result"><span id="${id}">___</span><label>${label}</label></span>`
                }
                text += `</p></div>`;
            }
        }

        if (!isInBrowser)
            return;

        for (let f = 0; f < 5; f++)
            text += `<div class="benchmark fill"></div>`;

        let timestamp = Date.now();
        document.getElementById('jetstreams').style.backgroundImage = `url('jetstreams.svg?${timestamp}')`;
        let resultsTable = document.getElementById("results");
        resultsTable.innerHTML = text;

        document.getElementById("magic").textContent = "";
        document.addEventListener('keypress', function (e) {
            if (e.which === 13)
                JetStream.start();
        });
    }

    reportError(benchmark)
    {
        for (let id of benchmark.scoreIdentifiers())
            document.getElementById(id).innerHTML = "error";
    }

    async initialize() {
        await this.fetchResources();
        this.prepareToRun();
        if (isInBrowser && window.location.search == '?report=true') {
            setTimeout(() => this.start(), 4000);
        }
    }

    async fetchResources() {
        for (let benchmark of this.benchmarks)
            await benchmark.fetchResources();

        if (!isInBrowser)
            return;

        let statusElement = document.getElementById("status");
        statusElement.classList.remove('loading');
        statusElement.innerHTML = `<a href="javascript:JetStream.start()" class="button">Start Test</a>`;
        statusElement.onclick = () => {
            statusElement.onclick = null;
            JetStream.start();
            return false;
        }
    }

    async reportScoreToRunBenchmarkRunner()
    {
        if (!isInBrowser)
            return;

        if (window.location.search !== '?report=true')
            return;

        let results = {};
        for (let benchmark of this.benchmarks) {
            const subResults = {}
            const subTimes = benchmark.subTimes();
            for (const name in subTimes) {
                subResults[name] = {"metrics": {"Time": {"current": [toTimeValue(subTimes[name])]}}};
            }
            results[benchmark.name] = {
                "metrics" : {
                    "Score" : {"current" : [benchmark.score]},
                    "Time": ["Geometric"],
                },
                "tests": subResults,
            };;
        }

        results = {"JetStream2.0": {"metrics" : {"Score" : ["Geometric"]}, "tests" : results}};

        const content = JSON.stringify(results);
        await fetch("/report", {
            method: "POST",
            heeaders: {
                "Content-Type": "application/json",
                "Content-Length": content.length,
                "Connection": "close",
            },
            body: content,
        });
    }
};

class Benchmark {
    constructor(plan)
    {
        this.plan = plan;
        this.iterations = testIterationCount || plan.iterations || defaultIterationCount;
        this.isAsync = !!plan.isAsync;

        this.scripts = null;

        this._resourcesPromise = Promise.resolve();
        this.scripts = this.plan.files.map((file) => readFile(file));
    }

    get name() { return this.plan.name; }

    get runnerCode() {
        return `
            let __benchmark = new Benchmark(${this.iterations});
            let results = [];
            for (let i = 0; i < ${this.iterations}; i++) {
                if (__benchmark.prepareForNextIteration)
                    __benchmark.prepareForNextIteration();

                let __jetstreamIterationStart = Date.now();
                __benchmark.runIteration();
                let __jetstreamIterationEnd = Date.now();

                results.push(Math.max(1, __jetstreamIterationEnd - __jetstreamIterationStart));
            }
            if (__benchmark.validate)
                __benchmark.validate();
            top.currentResolve(results);`;
    }

    processResults() {
        throw new Error("Subclasses need to implement this");
    }

    get score() {
        throw new Error("Subclasses need to implement this");
    }

    get prerunCode() { return null; }

    async run() {
        let code;
        if (isInBrowser)
            code = "";
        else
            code = [];

        let addScript = (text) => {
            if (isInBrowser)
                code += `<script>${text}</script>`;
            else
                code.push(text);
        };

        let addScriptWithURL = (url) => {
            if (isInBrowser)
                code += `<script src="${url}"></script>`;
            else
                assert(false, "Should not reach here in CLI");
        };

        addScript(`var performance = globalThis.performance = {now: Date.now.bind(Date)};`);

        if (!!this.plan.deterministicRandom) {
            addScript(`
                Math.random = (function() {
                    var seed = 49734321;
                    return function() {
                        // Robert Jenkins' 32 bit integer hash function.
                        seed = ((seed + 0x7ed55d16) + (seed << 12))  & 0xffffffff;
                        seed = ((seed ^ 0xc761c23c) ^ (seed >>> 19)) & 0xffffffff;
                        seed = ((seed + 0x165667b1) + (seed << 5))   & 0xffffffff;
                        seed = ((seed + 0xd3a2646c) ^ (seed << 9))   & 0xffffffff;
                        seed = ((seed + 0xfd7046c5) + (seed << 3))   & 0xffffffff;
                        seed = ((seed ^ 0xb55a4f09) ^ (seed >>> 16)) & 0xffffffff;
                        return (seed & 0xfffffff) / 0x10000000;
                    };
                })();
            `);

        }

        if (this.plan.preload) {
            let str = "";
            for (let [variableName, blobUrl] of this.preloads)
                str += `const ${variableName} = "${blobUrl}";\n`;
            addScript(str);
        }

        let prerunCode = this.prerunCode;
        if (prerunCode)
            addScript(prerunCode);

        if (preloadResources) {
            assert(this.scripts && this.scripts.length === this.plan.files.length);

            for (let text of this.scripts)
                addScript(text);
        } else {
            for (let file of this.plan.files)
                addScriptWithURL(file);
        }

        let promise = new Promise((resolve, reject) => {
            currentResolve = resolve;
            currentReject = reject;
        });

        if (isInBrowser) {
            code = `
                <script> window.onerror = top.currentReject; </script>
                ${code}
            `;
        }
        addScript("(() => {\n" + this.runnerCode + "\n})();");

        this.startTime = new Date();

        if (RAMification)
            resetMemoryPeak();

        let magicFrame;
        try {
            magicFrame = JetStream.runCode(code);
        } catch(e) {
            console.log("Error in runCode: ", e);
            throw e;
        }
        let results = await promise;

        this.endTime = new Date();

        if (RAMification) {
            let memoryFootprint = MemoryFootprint();
            this.currentFootprint = memoryFootprint.current;
            this.peakFootprint = memoryFootprint.peak;
        }

        this.processResults(results);
        if (isInBrowser)
            magicFrame.contentDocument.close();
    }

    fetchResources() {
        if (this._resourcesPromise)
            return this._resourcesPromise;

        let filePromises = preloadResources ? this.plan.files.map((file) => fileLoader.load(file)) : [];
        let preloads = [];
        let preloadVariableNames = [];

        if (isInBrowser && this.plan.preload) {
            for (let prop of Object.getOwnPropertyNames(this.plan.preload)) {
                preloadVariableNames.push(prop);
                preloads.push(this.plan.preload[prop]);
            }
        }

        preloads = preloads.map((file) => fileLoader.load(file));

        let p1 = Promise.all(filePromises).then((texts) => {
            if (!preloadResources)
                return;
            this.scripts = [];
            assert(texts.length === this.plan.files.length);
            for (let text of texts)
                this.scripts.push(text);
        });

        let p2 = Promise.all(preloads).then((data) => {
            this.preloads = [];
            this.blobs = [];
            for (let i = 0; i < data.length; ++i) {
                let item = data[i];

                let blob;
                if (typeof item === "string") {
                    blob = new Blob([item], {type : 'application/javascript'});
                } else if (item instanceof ArrayBuffer) {
                    blob = new Blob([item], {type : 'application/octet-stream'});
                } else
                    throw new Error("Unexpected item!");

                this.blobs.push(blob);
                this.preloads.push([preloadVariableNames[i], URL.createObjectURL(blob)]);
            }
        });

        this._resourcesPromise = Promise.all([p1, p2]);
        return this._resourcesPromise;
    }

    static scoreDescription() { throw new Error("Must be implemented by subclasses."); }
    scoreIdentifiers() { throw new Error("Must be implemented by subclasses"); }

    updateUIBeforeRun() {
        if (!isInBrowser) {
            console.log(`Running ${this.name}:`);
            return;
        }

        let containerUI = document.getElementById("results");
        let resultsBenchmarkUI = document.getElementById(`benchmark-${this.name}`);
        containerUI.insertBefore(resultsBenchmarkUI, containerUI.firstChild);
        resultsBenchmarkUI.classList.add("benchmark-running");

        for (let id of this.scoreIdentifiers())
            document.getElementById(id).innerHTML = "...";
    }

    updateUIAfterRun() {
        if (!isInBrowser)
            return;

        let benchmarkResultsUI = document.getElementById(`benchmark-${this.name}`);
        benchmarkResultsUI.classList.remove("benchmark-running");
        benchmarkResultsUI.classList.add("benchmark-done");

    }
};

class DefaultBenchmark extends Benchmark {
    constructor(...args) {
        super(...args);

        this.worstCaseCount = this.plan.worstCaseCount || defaultWorstCaseCount;
        this.firstIteration = null;
        this.worst4 = null;
        this.average = null;
    }

    processResults(results) {
        function copyArray(a) {
            let result = [];
            for (let x of a)
                result.push(x);
            return result;
        }
        results = copyArray(results);

        this.firstIteration = toScore(results[0]);

        results = results.slice(1);
        results.sort((a, b) => a < b ? 1 : -1);
        for (let i = 0; i + 1 < results.length; ++i)
            assert(results[i] >= results[i + 1]);

        let worstCase = [];
        for (let i = 0; i < this.worstCaseCount; ++i)
            worstCase.push(results[i]);
        this.worst4 = toScore(mean(worstCase));
        this.average = toScore(mean(results));
    }

    get score() {
        return geomean([this.firstIteration, this.worst4, this.average]);
    }

    subTimes() {
        return {
            "First": this.firstIteration,
            "Worst": this.worst4,
            "Average": this.average,
        };
    }

    static scoreDescription() {
        return ["First", "Worst", "Average", "Score"];
    }

    scoreIdentifiers() {
        return [firstID(this), worst4ID(this), avgID(this), scoreID(this)];
    }

    updateUIAfterRun() {
        super.updateUIAfterRun();

        if (isInBrowser) {
            document.getElementById(firstID(this)).innerHTML = uiFriendlyNumber(this.firstIteration);
            document.getElementById(worst4ID(this)).innerHTML = uiFriendlyNumber(this.worst4);
            document.getElementById(avgID(this)).innerHTML = uiFriendlyNumber(this.average);
            document.getElementById(scoreID(this)).innerHTML = uiFriendlyNumber(this.score);
            return;
        }

        print("    Startup:", uiFriendlyNumber(this.firstIteration));
        print("    Worst Case:", uiFriendlyNumber(this.worst4));
        print("    Average:", uiFriendlyNumber(this.average));
        print("    Score:", uiFriendlyNumber(this.score));
        if (RAMification) {
            print("    Current Footprint:", uiFriendlyNumber(this.currentFootprint));
            print("    Peak Footprint:", uiFriendlyNumber(this.peakFootprint));
        }
        print("    Wall time:", uiFriendlyDuration(new Date(this.endTime - this.startTime)));
    }
}

class AsyncBenchmark extends DefaultBenchmark {
    get runnerCode() {
        return `
        async function doRun() {
            let __benchmark = new Benchmark();
            let results = [];
            for (let i = 0; i < ${this.iterations}; i++) {
                let start = Date.now();
                await __benchmark.runIteration();
                let end = Date.now();
                results.push(Math.max(1, end - start));
            }
            if (__benchmark.validate)
                __benchmark.validate();
            top.currentResolve(results);
        }
        doRun();`
    }
};

class WSLBenchmark extends Benchmark {
    constructor(...args) {
        super(...args);

        this.stdlib = null;
        this.mainRun = null;
    }

    processResults(results) {
        this.stdlib = toScore(results[0]);
        this.mainRun = toScore(results[1]);
    }

    get score() {
        return geomean([this.stdlib, this.mainRun]);
    }

    get runnerCode() {
        return `
            let benchmark = new Benchmark();
            let results = [];
            {
                let start = Date.now();
                benchmark.buildStdlib();
                results.push(Date.now() - start);
            }

            {
                let start = Date.now();
                benchmark.run();
                results.push(Date.now() - start);
            }

            top.currentResolve(results);
            `;
    }

    subTimes() {
        return {
            "Stdlib": this.stdlib,
            "MainRun": this.mainRun,
        };
    }

    static scoreDescription() {
        return ["Stdlib", "MainRun", "Score"];
    }

    scoreIdentifiers() {
        return ["wsl-stdlib-score", "wsl-tests-score", "wsl-score-score"];
    }

    updateUIAfterRun() {
        super.updateUIAfterRun();

        if (isInBrowser) {
            document.getElementById("wsl-stdlib-score").innerHTML = uiFriendlyNumber(this.stdlib);
            document.getElementById("wsl-tests-score").innerHTML = uiFriendlyNumber(this.mainRun);
            document.getElementById("wsl-score-score").innerHTML = uiFriendlyNumber(this.score);
            return;
        }

        print("    Stdlib:", uiFriendlyNumber(this.stdlib));
        print("    Tests:", uiFriendlyNumber(this.mainRun));
        print("    Score:", uiFriendlyNumber(this.score));
        if (RAMification) {
            print("    Current Footprint:", uiFriendlyNumber(this.currentFootprint));
            print("    Peak Footprint:", uiFriendlyNumber(this.peakFootprint));
        }
        print("    Wall time:", uiFriendlyDuration(new Date(this.endTime - this.startTime)));
    }
};

class WasmBenchmark extends Benchmark {
    constructor(...args) {
        super(...args);

        this.startupTime = null;
        this.runTime = null;
    }

    processResults(results) {
        this.startupTime = toScore(results[0]);
        this.runTime = toScore(results[1]);
    }

    get score() {
        return geomean([this.startupTime, this.runTime]);
    }

    get wasmPath() {
        return this.plan.wasmPath;
    }

    get prerunCode() {
        let str = `
            let verbose = false;

            let compileTime = null;
            let runTime = null;

            let globalObject = this;

            globalObject.benchmarkTime = Date.now.bind(Date);

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

            oldPrint = globalObject.print;
            globalObject.print = globalObject.printErr = (...args) => {
                if (verbose)
                    console.log('Intercepted print: ', ...args);
            };

            let Module = {
                preRun: [],
                postRun: [],
                print: function() { },
                printErr: function() { },
                setStatus: function(text) {
                },
                totalDependencies: 0,
                monitorRunDependencies: function(left) {
                    this.totalDependencies = Math.max(this.totalDependencies, left);
                    Module.setStatus(left ? 'Preparing... (' + (this.totalDependencies-left) + '/' + this.totalDependencies + ')' : 'All downloads complete.');
                }
            };
            globalObject.Module = Module;
            `;
        return str;
    }

    get runnerCode() {
        let str = "";
        if (isInBrowser) {
            str += `
                var xhr = new XMLHttpRequest();
                xhr.open('GET', wasmBlobURL, true);
                xhr.responseType = 'arraybuffer';
                xhr.onload = function() {
                    Module.wasmBinary = xhr.response;
                    doRun();
                };
                xhr.send(null);
            `;
        } else {
            str += `
            Module.wasmBinary = read("${this.wasmPath}", "binary");
            globalObject.read = (...args) => {
                console.log("should not be inside read: ", ...args);
                throw new Error;
            };

            Module.setStatus = null;
            Module.monitorRunDependencies = null;

            Promise.resolve(42).then(() => {
                try {
                    doRun();
                } catch(e) {
                    console.log("error running wasm:", e);
                    throw e;
                }
            })
            `;
        }
        return str;
    }

    subTimes() {
        return {
            "Startup": this.startupTime,
            "Runtime": this.runTime,
        };
    }

    static scoreDescription() {
        return ["Startup", "Runtime", "Score"];
    }

    get startupID() {
        return `wasm-startup-id${this.name}`;
    }
    get runID() {
        return `wasm-run-id${this.name}`;
    }
    get scoreID() {
        return `wasm-score-id${this.name}`;
    }

    scoreIdentifiers() {
        return [this.startupID, this.runID, this.scoreID];
    }

    updateUIAfterRun() {
        super.updateUIAfterRun();

        if (isInBrowser) {
            document.getElementById(this.startupID).innerHTML = uiFriendlyNumber(this.startupTime);
            document.getElementById(this.runID).innerHTML = uiFriendlyNumber(this.runTime);
            document.getElementById(this.scoreID).innerHTML = uiFriendlyNumber(this.score);
            return;
        }
        print("    Startup:", uiFriendlyNumber(this.startupTime));
        print("    Run time:", uiFriendlyNumber(this.runTime));
        if (RAMification) {
            print("    Current Footprint:", uiFriendlyNumber(this.currentFootprint));
            print("    Peak Footprint:", uiFriendlyNumber(this.peakFootprint));
        }
        print("    Score:", uiFriendlyNumber(this.score));
    }
};

const ARESGroup = Symbol.for("ARES");
const CDJSGroup = Symbol.for("CDJS");
const CodeLoadGroup = Symbol.for("CodeLoad");
const LuaJSFightGroup = Symbol.for("LuaJSFight");
const OctaneGroup = Symbol.for("Octane");
const RexBenchGroup = Symbol.for("RexBench");
const SeaMonsterGroup = Symbol.for("SeaMonster");
const SimpleGroup = Symbol.for("Simple");
const SunSpiderGroup = Symbol.for("SunSpider");
const WasmGroup = Symbol.for("Wasm");
const WorkerTestsGroup = Symbol.for("WorkerTests");
const WSLGroup = Symbol.for("WSL");
const WTBGroup = Symbol.for("WTB");


let testPlans = [
    // ARES
    {
        name: "Air",
        files: [
            "./ARES-6/Air/symbols.js"
            , "./ARES-6/Air/tmp_base.js"
            , "./ARES-6/Air/arg.js"
            , "./ARES-6/Air/basic_block.js"
            , "./ARES-6/Air/code.js"
            , "./ARES-6/Air/frequented_block.js"
            , "./ARES-6/Air/inst.js"
            , "./ARES-6/Air/opcode.js"
            , "./ARES-6/Air/reg.js"
            , "./ARES-6/Air/stack_slot.js"
            , "./ARES-6/Air/tmp.js"
            , "./ARES-6/Air/util.js"
            , "./ARES-6/Air/custom.js"
            , "./ARES-6/Air/liveness.js"
            , "./ARES-6/Air/insertion_set.js"
            , "./ARES-6/Air/allocate_stack.js"
            , "./ARES-6/Air/payload-gbemu-executeIteration.js"
            , "./ARES-6/Air/payload-imaging-gaussian-blur-gaussianBlur.js"
            , "./ARES-6/Air/payload-airjs-ACLj8C.js"
            , "./ARES-6/Air/payload-typescript-scanIdentifier.js"
            , "./ARES-6/Air/benchmark.js"
        ],
        testGroup: ARESGroup
    },
    {
        name: "Basic",
        files: [
            "./ARES-6/Basic/ast.js"
            , "./ARES-6/Basic/basic.js"
            , "./ARES-6/Basic/caseless_map.js"
            , "./ARES-6/Basic/lexer.js"
            , "./ARES-6/Basic/number.js"
            , "./ARES-6/Basic/parser.js"
            , "./ARES-6/Basic/random.js"
            , "./ARES-6/Basic/state.js"
            , "./ARES-6/Basic/util.js"
            , "./ARES-6/Basic/benchmark.js"
        ],
        testGroup: ARESGroup
    },
    {
        name: "ML",
        files: [
            "./ARES-6/ml/index.js"
            , "./ARES-6/ml/benchmark.js"
        ],
        iterations: 60,
        testGroup: ARESGroup
    },
    {
        name: "Babylon",
        files: [
            "./ARES-6/Babylon/index.js"
            , "./ARES-6/Babylon/benchmark.js"
        ],
        preload: {
            airBlob: "./ARES-6/Babylon/air-blob.js",
            basicBlob: "./ARES-6/Babylon/basic-blob.js",
            inspectorBlob: "./ARES-6/Babylon/inspector-blob.js",
            babylonBlob: "./ARES-6/Babylon/babylon-blob.js"
        },
        testGroup: ARESGroup
    },
    // CDJS
    {
        name: "cdjs",
        files: [
            "./cdjs/constants.js"
            , "./cdjs/util.js"
            , "./cdjs/red_black_tree.js"
            , "./cdjs/call_sign.js"
            , "./cdjs/vector_2d.js"
            , "./cdjs/vector_3d.js"
            , "./cdjs/motion.js"
            , "./cdjs/reduce_collision_set.js"
            , "./cdjs/simulator.js"
            , "./cdjs/collision.js"
            , "./cdjs/collision_detector.js"
            , "./cdjs/benchmark.js"
        ],
        iterations: 60,
        worstCaseCount: 3,
        testGroup: CDJSGroup
    },
    // CodeLoad
    {
        name: "first-inspector-code-load",
        files: [
            "./code-load/code-first-load.js"
        ],
        preload: {
            inspectorPayloadBlob: "./code-load/inspector-payload-minified.js"
        },
        testGroup: CodeLoadGroup
    },
    {
        name: "multi-inspector-code-load",
        files: [
            "./code-load/code-multi-load.js"
        ],
        preload: {
            inspectorPayloadBlob: "./code-load/inspector-payload-minified.js"
        },
        testGroup: CodeLoadGroup
    },
    // Octane
    {
        name: "Box2D",
        files: [
            "./Octane/box2d.js"
        ],
        deterministicRandom: true,
        testGroup: OctaneGroup
    },
    {
        name: "octane-code-load",
        files: [
            "./Octane/code-first-load.js"
        ],
        deterministicRandom: true,
        testGroup: OctaneGroup
    },
    {
        name: "crypto",
        files: [
            "./Octane/crypto.js"
        ],
        deterministicRandom: true,
        testGroup: OctaneGroup
    },
    {
        name: "delta-blue",
        files: [
            "./Octane/deltablue.js"
        ],
        deterministicRandom: true,
        testGroup: OctaneGroup
    },
    {
        name: "earley-boyer",
        files: [
            "./Octane/earley-boyer.js"
        ],
        deterministicRandom: true,
        testGroup: OctaneGroup
    },
    {
        name: "gbemu",
        files: [
            "./Octane/gbemu-part1.js"
            , "./Octane/gbemu-part2.js"
        ],
        deterministicRandom: true,
        testGroup: OctaneGroup
    },
    {
        name: "mandreel",
        files: [
            "./Octane/mandreel.js"
        ],
        iterations: 80,
        deterministicRandom: true,
        testGroup: OctaneGroup
    },
    {
        name: "navier-stokes",
        files: [
            "./Octane/navier-stokes.js"
        ],
        deterministicRandom: true,
        testGroup: OctaneGroup
    },
    {
        name: "pdfjs",
        files: [
            "./Octane/pdfjs.js"
        ],
        deterministicRandom: true,
        testGroup: OctaneGroup
    },
    {
        name: "raytrace",
        files: [
            "./Octane/raytrace.js"
        ],
        deterministicRandom: true,
        testGroup: OctaneGroup
    },
    {
        name: "regexp",
        files: [
            "./Octane/regexp.js"
        ],
        deterministicRandom: true,
        testGroup: OctaneGroup
    },
    {
        name: "richards",
        files: [
            "./Octane/richards.js"
        ],
        deterministicRandom: true,
        testGroup: OctaneGroup
    },
    {
        name: "splay",
        files: [
            "./Octane/splay.js"
        ],
        deterministicRandom: true,
        testGroup: OctaneGroup
    },
    {
        name: "typescript",
        files: [
            "./Octane/typescript-compiler.js"
            , "./Octane/typescript-input.js"
            , "./Octane/typescript.js"
        ],
        iterations: 15,
        worstCaseCount: 2,
        deterministicRandom: true,
        testGroup: OctaneGroup
    },
    {
        name: "octane-zlib",
        files: [
            "./Octane/zlib-data.js"
            , "./Octane/zlib.js"
        ],
        iterations: 15,
        worstCaseCount: 2,
        deterministicRandom: true,
        testGroup: OctaneGroup
    },
    // RexBench
    {
        name: "FlightPlanner",
        files: [
            "./RexBench/FlightPlanner/airways.js"
            , "./RexBench/FlightPlanner/waypoints.js"
            , "./RexBench/FlightPlanner/flight_planner.js"
            , "./RexBench/FlightPlanner/expectations.js"
            , "./RexBench/FlightPlanner/benchmark.js"
        ],
        testGroup: RexBenchGroup
    },
    {
        name: "OfflineAssembler",
        files: [
            "./RexBench/OfflineAssembler/registers.js"
            , "./RexBench/OfflineAssembler/instructions.js"
            , "./RexBench/OfflineAssembler/ast.js"
            , "./RexBench/OfflineAssembler/parser.js"
            , "./RexBench/OfflineAssembler/file.js"
            , "./RexBench/OfflineAssembler/LowLevelInterpreter.js"
            , "./RexBench/OfflineAssembler/LowLevelInterpreter32_64.js"
            , "./RexBench/OfflineAssembler/LowLevelInterpreter64.js"
            , "./RexBench/OfflineAssembler/InitBytecodes.js"
            , "./RexBench/OfflineAssembler/expected.js"
            , "./RexBench/OfflineAssembler/benchmark.js"
        ],
        iterations: 80,
        testGroup: RexBenchGroup
    },
    {
        name: "UniPoker",
        files: [
            "./RexBench/UniPoker/poker.js"
            , "./RexBench/UniPoker/expected.js"
            , "./RexBench/UniPoker/benchmark.js"
        ],
        deterministicRandom: true,
        testGroup: RexBenchGroup
    },
    // Simple
    {
        name: "async-fs",
        files: [
            "./simple/file-system.js"
        ],
        iterations: 40,
        worstCaseCount: 3,
        benchmarkClass: AsyncBenchmark,
        testGroup: SimpleGroup
    },
    {
        name: "float-mm.c",
        files: [
            "./simple/float-mm.c.js"
        ],
        iterations: 15,
        worstCaseCount: 2,
        testGroup: SimpleGroup
    },
    {
        name: "hash-map",
        files: [
            "./simple/hash-map.js"
        ],
        testGroup: SimpleGroup
    },
    // SeaMonster
    {
        name: "ai-astar",
        files: [
            "./SeaMonster/ai-astar.js"
        ],
        testGroup: SeaMonsterGroup
    },
    {
        name: "gaussian-blur",
        files: [
            "./SeaMonster/gaussian-blur.js"
        ],
        testGroup: SeaMonsterGroup
    },
    {
        name: "stanford-crypto-aes",
        files: [
            "./SeaMonster/sjlc.js"
            , "./SeaMonster/stanford-crypto-aes.js"
        ],
        testGroup: SeaMonsterGroup
    },
    {
        name: "stanford-crypto-pbkdf2",
        files: [
            "./SeaMonster/sjlc.js"
            , "./SeaMonster/stanford-crypto-pbkdf2.js"
        ],
        testGroup: SeaMonsterGroup
    },
    {
        name: "stanford-crypto-sha256",
        files: [
            "./SeaMonster/sjlc.js"
            , "./SeaMonster/stanford-crypto-sha256.js"
        ],
        testGroup: SeaMonsterGroup
    },
    {
        name: "json-stringify-inspector",
        files: [
            "./SeaMonster/inspector-json-payload.js"
            , "./SeaMonster/json-stringify-inspector.js"
        ],
        iterations: 20,
        worstCaseCount: 2,
        testGroup: SeaMonsterGroup
    },
    {
        name: "json-parse-inspector",
        files: [
            "./SeaMonster/inspector-json-payload.js"
            , "./SeaMonster/json-parse-inspector.js"
        ],
        iterations: 20,
        worstCaseCount: 2,
        testGroup: SeaMonsterGroup
    },
    // Wasm
    {
        name: "HashSet-wasm",
        wasmPath: "./wasm/HashSet.wasm",
        files: [
            "./wasm/HashSet.js"
        ],
        preload: {
            wasmBlobURL: "./wasm/HashSet.wasm"
        },
        benchmarkClass: WasmBenchmark,
        testGroup: WasmGroup
    },
    {
        name: "tsf-wasm",
        wasmPath: "./wasm/tsf.wasm",
        files: [
            "./wasm/tsf.js"
        ],
        preload: {
            wasmBlobURL: "./wasm/tsf.wasm"
        },
        benchmarkClass: WasmBenchmark,
        testGroup: WasmGroup
    },
    {
        name: "quicksort-wasm",
        wasmPath: "./wasm/quicksort.wasm",
        files: [
            "./wasm/quicksort.js"
        ],
        preload: {
            wasmBlobURL: "./wasm/quicksort.wasm"
        },
        benchmarkClass: WasmBenchmark,
        testGroup: WasmGroup
    },
    {
        name: "gcc-loops-wasm",
        wasmPath: "./wasm/gcc-loops.wasm",
        files: [
            "./wasm/gcc-loops.js"
        ],
        preload: {
            wasmBlobURL: "./wasm/gcc-loops.wasm"
        },
        benchmarkClass: WasmBenchmark,
        testGroup: WasmGroup
    },
    {
        name: "richards-wasm",
        wasmPath: "./wasm/richards.wasm",
        files: [
            "./wasm/richards.js"
        ],
        preload: {
            wasmBlobURL: "./wasm/richards.wasm"
        },
        benchmarkClass: WasmBenchmark,
        testGroup: WasmGroup
    },
    // WorkerTests
    {
        name: "bomb-workers",
        files: [
            "./worker/bomb.js"
        ],
        iterations: 80,
        preload: {
            rayTrace3D: "./worker/bomb-subtests/3d-raytrace.js"
            , accessNbody: "./worker/bomb-subtests/access-nbody.js"
            , morph3D: "./worker/bomb-subtests/3d-morph.js"
            , cube3D: "./worker/bomb-subtests/3d-cube.js"
            , accessFunnkuch: "./worker/bomb-subtests/access-fannkuch.js"
            , accessBinaryTrees: "./worker/bomb-subtests/access-binary-trees.js"
            , accessNsieve: "./worker/bomb-subtests/access-nsieve.js"
            , bitopsBitwiseAnd: "./worker/bomb-subtests/bitops-bitwise-and.js"
            , bitopsNsieveBits: "./worker/bomb-subtests/bitops-nsieve-bits.js"
            , controlflowRecursive: "./worker/bomb-subtests/controlflow-recursive.js"
            , bitops3BitBitsInByte: "./worker/bomb-subtests/bitops-3bit-bits-in-byte.js"
            , botopsBitsInByte: "./worker/bomb-subtests/bitops-bits-in-byte.js"
            , cryptoAES: "./worker/bomb-subtests/crypto-aes.js"
            , cryptoMD5: "./worker/bomb-subtests/crypto-md5.js"
            , cryptoSHA1: "./worker/bomb-subtests/crypto-sha1.js"
            , dateFormatTofte: "./worker/bomb-subtests/date-format-tofte.js"
            , dateFormatXparb: "./worker/bomb-subtests/date-format-xparb.js"
            , mathCordic: "./worker/bomb-subtests/math-cordic.js"
            , mathPartialSums: "./worker/bomb-subtests/math-partial-sums.js"
            , mathSpectralNorm: "./worker/bomb-subtests/math-spectral-norm.js"
            , stringBase64: "./worker/bomb-subtests/string-base64.js"
            , stringFasta: "./worker/bomb-subtests/string-fasta.js"
            , stringValidateInput: "./worker/bomb-subtests/string-validate-input.js"
            , stringTagcloud: "./worker/bomb-subtests/string-tagcloud.js"
            , stringUnpackCode: "./worker/bomb-subtests/string-unpack-code.js"
            , regexpDNA: "./worker/bomb-subtests/regexp-dna.js"
        },
        benchmarkClass: AsyncBenchmark,
        testGroup: WorkerTestsGroup
    },
    {
        name: "segmentation",
        files: [
            "./worker/segmentation.js"
        ],
        preload: {
            asyncTaskBlob: "./worker/async-task.js"
        },
        iterations: 36,
        worstCaseCount: 3,
        benchmarkClass: AsyncBenchmark,
        testGroup: WorkerTestsGroup
    },
    // WSL
    {
        name: "WSL",
        files: ["./WSL/Node.js" ,"./WSL/Type.js" ,"./WSL/ReferenceType.js" ,"./WSL/Value.js" ,"./WSL/Expression.js" ,"./WSL/Rewriter.js" ,"./WSL/Visitor.js" ,"./WSL/CreateLiteral.js" ,"./WSL/CreateLiteralType.js" ,"./WSL/PropertyAccessExpression.js" ,"./WSL/AddressSpace.js" ,"./WSL/AnonymousVariable.js" ,"./WSL/ArrayRefType.js" ,"./WSL/ArrayType.js" ,"./WSL/Assignment.js" ,"./WSL/AutoWrapper.js" ,"./WSL/Block.js" ,"./WSL/BoolLiteral.js" ,"./WSL/Break.js" ,"./WSL/CallExpression.js" ,"./WSL/CallFunction.js" ,"./WSL/Check.js" ,"./WSL/CheckLiteralTypes.js" ,"./WSL/CheckLoops.js" ,"./WSL/CheckRecursiveTypes.js" ,"./WSL/CheckRecursion.js" ,"./WSL/CheckReturns.js" ,"./WSL/CheckUnreachableCode.js" ,"./WSL/CheckWrapped.js" ,"./WSL/Checker.js" ,"./WSL/CloneProgram.js" ,"./WSL/CommaExpression.js" ,"./WSL/ConstexprFolder.js" ,"./WSL/ConstexprTypeParameter.js" ,"./WSL/Continue.js" ,"./WSL/ConvertPtrToArrayRefExpression.js" ,"./WSL/DereferenceExpression.js" ,"./WSL/DoWhileLoop.js" ,"./WSL/DotExpression.js" ,"./WSL/DoubleLiteral.js" ,"./WSL/DoubleLiteralType.js" ,"./WSL/EArrayRef.js" ,"./WSL/EBuffer.js" ,"./WSL/EBufferBuilder.js" ,"./WSL/EPtr.js" ,"./WSL/EnumLiteral.js" ,"./WSL/EnumMember.js" ,"./WSL/EnumType.js" ,"./WSL/EvaluationCommon.js" ,"./WSL/Evaluator.js" ,"./WSL/ExpressionFinder.js" ,"./WSL/ExternalOrigin.js" ,"./WSL/Field.js" ,"./WSL/FindHighZombies.js" ,"./WSL/FlattenProtocolExtends.js" ,"./WSL/FlattenedStructOffsetGatherer.js" ,"./WSL/FloatLiteral.js" ,"./WSL/FloatLiteralType.js" ,"./WSL/FoldConstexprs.js" ,"./WSL/ForLoop.js" ,"./WSL/Func.js" ,"./WSL/FuncDef.js" ,"./WSL/FuncInstantiator.js" ,"./WSL/FuncParameter.js" ,"./WSL/FunctionLikeBlock.js" ,"./WSL/HighZombieFinder.js" ,"./WSL/IdentityExpression.js" ,"./WSL/IfStatement.js" ,"./WSL/IndexExpression.js" ,"./WSL/InferTypesForCall.js" ,"./WSL/Inline.js" ,"./WSL/Inliner.js" ,"./WSL/InstantiateImmediates.js" ,"./WSL/IntLiteral.js" ,"./WSL/IntLiteralType.js" ,"./WSL/Intrinsics.js" ,"./WSL/LateChecker.js" ,"./WSL/Lexer.js" ,"./WSL/LexerToken.js" ,"./WSL/LiteralTypeChecker.js" ,"./WSL/LogicalExpression.js" ,"./WSL/LogicalNot.js" ,"./WSL/LoopChecker.js" ,"./WSL/MakeArrayRefExpression.js" ,"./WSL/MakePtrExpression.js" ,"./WSL/NameContext.js" ,"./WSL/NameFinder.js" ,"./WSL/NameResolver.js" ,"./WSL/NativeFunc.js" ,"./WSL/NativeFuncInstance.js" ,"./WSL/NativeType.js" ,"./WSL/NativeTypeInstance.js" ,"./WSL/NormalUsePropertyResolver.js" ,"./WSL/NullLiteral.js" ,"./WSL/NullType.js" ,"./WSL/OriginKind.js" ,"./WSL/OverloadResolutionFailure.js" ,"./WSL/Parse.js" ,"./WSL/Prepare.js" ,"./WSL/Program.js" ,"./WSL/ProgramWithUnnecessaryThingsRemoved.js" ,"./WSL/PropertyResolver.js" ,"./WSL/Protocol.js" ,"./WSL/ProtocolDecl.js" ,"./WSL/ProtocolFuncDecl.js" ,"./WSL/ProtocolRef.js" ,"./WSL/PtrType.js" ,"./WSL/ReadModifyWriteExpression.js" ,"./WSL/RecursionChecker.js" ,"./WSL/RecursiveTypeChecker.js" ,"./WSL/ResolveNames.js" ,"./WSL/ResolveOverloadImpl.js" ,"./WSL/ResolveProperties.js" ,"./WSL/ResolveTypeDefs.js" ,"./WSL/Return.js" ,"./WSL/ReturnChecker.js" ,"./WSL/ReturnException.js" ,"./WSL/StandardLibrary.js" ,"./WSL/StatementCloner.js" ,"./WSL/StructLayoutBuilder.js" ,"./WSL/StructType.js" ,"./WSL/Substitution.js" ,"./WSL/SwitchCase.js" ,"./WSL/SwitchStatement.js" ,"./WSL/SynthesizeEnumFunctions.js" ,"./WSL/SynthesizeStructAccessors.js" ,"./WSL/TrapStatement.js" ,"./WSL/TypeDef.js" ,"./WSL/TypeDefResolver.js" ,"./WSL/TypeOrVariableRef.js" ,"./WSL/TypeParameterRewriter.js" ,"./WSL/TypeRef.js" ,"./WSL/TypeVariable.js" ,"./WSL/TypeVariableTracker.js" ,"./WSL/TypedValue.js" ,"./WSL/UintLiteral.js" ,"./WSL/UintLiteralType.js" ,"./WSL/UnificationContext.js" ,"./WSL/UnreachableCodeChecker.js" ,"./WSL/VariableDecl.js" ,"./WSL/VariableRef.js" ,"./WSL/VisitingSet.js" ,"./WSL/WSyntaxError.js" ,"./WSL/WTrapError.js" ,"./WSL/WTypeError.js" ,"./WSL/WhileLoop.js" ,"./WSL/WrapChecker.js", "./WSL/Test.js"],
        benchmarkClass: WSLBenchmark,
        testGroup: WSLGroup
    }
];

// LuaJSFight tests
let luaJSFightTests = [
    "hello_world"
    , "list_search"
    , "lists"
    , "string_lists"
];
for (let test of luaJSFightTests) {
    testPlans.push({
        name: `${test}-LJF`,
        files: [
            `./LuaJSFight/${test}.js`
        ],
        testGroup: LuaJSFightGroup
    });
}

// SunSpider tests
let sunSpiderTests = [
    "3d-cube"
    , "3d-raytrace"
    , "base64"
    , "crypto-aes"
    , "crypto-md5"
    , "crypto-sha1"
    , "date-format-tofte"
    , "date-format-xparb"
    , "n-body"
    , "regex-dna"
    , "string-unpack-code"
    , "tagcloud"
];
for (let test of sunSpiderTests) {
    testPlans.push({
        name: `${test}-SP`,
        files: [
            `./SunSpider/${test}.js`
        ],
        testGroup: SunSpiderGroup
    });
}

// WTB (Web Tooling Benchmark) tests
let WTBTests = [
    "acorn"
    , "babylon"
    , "chai"
    , "coffeescript"
    , "espree"
    , "jshint"
    , "lebab"
    , "prepack"
    , "uglify-js"
];
for (let name of WTBTests) {
    testPlans.push({
        name: `${name}-wtb`,
        files: [
            isInBrowser ? "./web-tooling-benchmark/browser.js" : "./web-tooling-benchmark/cli.js"
            , `./web-tooling-benchmark/${name}.js`
        ],
        iterations: 5,
        worstCaseCount: 1,
        testGroup: WTBGroup
    });
}


let testsByName = new Map();
let testsByGroup = new Map();

for (let plan of testPlans) {
    let testName = plan.name;

    if (testsByName.has(plan.name))
        throw "Duplicate test plan with name \"" + testName + "\"";
    else
        testsByName.set(testName, plan);

    let group = plan.testGroup;

    if (testsByGroup.has(group))
        testsByGroup.get(group).push(testName);
    else
        testsByGroup.set(group, [testName]);
}

var JetStream = new Driver();

function addTestByName(testName)
{
    let plan = testsByName.get(testName);

    if (plan)
        JetStream.addPlan(plan, plan.benchmarkClass);
    else
        throw "Couldn't find test named \"" +  testName + "\"";
}

function addTestsByGroup(group)
{
    let testList = testsByGroup.get(group);

    if (!testList)
        throw "Couldn't find test group named: \"" + Symbol.keyFor(group) + "\"";

    for (let testName of testList)
        addTestByName(testName);
}

function processTestList(testList)
{
    let tests = [];

    if (testList instanceof Array)
        tests = testList;
    else
        tests = testList.split(/[\s,]/);

    for (let testName of tests) {
        let groupTest = testsByGroup.get(Symbol.for(testName));

        if (groupTest) {
            for (let testName of groupTest)
                addTestByName(testName);
        } else
            addTestByName(testName);
    }
}

let runOctane = true;
let runARES = true;
let runWSL = true;
let runRexBench = true;
let runWTB = true;
let runSunSpider = true;
let runSimple = true;
let runCDJS = true;
let runWorkerTests = !!isInBrowser;
let runSeaMonster = true;
let runCodeLoad = true;
let runWasm = true;

if (false) {
    runOctane = false;
    runARES = false;
    runWSL = false;
    runRexBench = false;
    runWTB = false;
    runSunSpider = false;
    runSimple = false;
    runCDJS = false;
    runWorkerTests = false;
    runSeaMonster = false;
    runCodeLoad = false;
    runWasm = false;
}

if (typeof testList !== "undefined") {
    processTestList(testList);
} else {
    if (runARES)
        addTestsByGroup(ARESGroup);

    if (runCDJS)
        addTestsByGroup(CDJSGroup);

    if (runCodeLoad)
        addTestsByGroup(CodeLoadGroup);

    if (runOctane)
        addTestsByGroup(OctaneGroup);

    if (runRexBench)
        addTestsByGroup(RexBenchGroup);

    if (runSeaMonster)
        addTestsByGroup(SeaMonsterGroup);

    if (runSimple)
        addTestsByGroup(SimpleGroup);

    if (runSunSpider)
        addTestsByGroup(SunSpiderGroup);

    if (runWasm)
        addTestsByGroup(WasmGroup);

    if (runWorkerTests)
        addTestsByGroup(WorkerTestsGroup);

    if (runWSL)
        addTestsByGroup(WSLGroup);

    if (runWTB)
        addTestsByGroup(WTBGroup);
}


JetStream.initialize()
    .then(() => JetStream.start())
    .catch((error) => print("JetStream2 failed:", error && error.stack ? error.stack : error));
undefined;
