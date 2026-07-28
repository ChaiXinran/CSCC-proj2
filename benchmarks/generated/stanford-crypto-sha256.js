
const isInBrowser = false;
var console = { log: (...args) => print(...args) };
var document = globalThis.document = {
    getElementById() { return { innerHTML: "" }; }
};
var testList = "stanford-crypto-sha256";
var testIterationCount = 1;
var RAMification = false;
var JetStreamParams = {
    prefetchResources: false,
    forceGC: false,
    dumpJSONResults: false,
    testIterationCountMap: {},
    testWorstCaseCountMap: {},
    testList: "stanford-crypto-sha256",
};
var __jetstreamResources = {"./SeaMonster/sjlc.js":"\"use strict\";\r\n/*\r\n * SJCL is open. You can use, modify and redistribute it under a BSD\r\n * license or under the GNU GPL, version 2.0.\r\n * \r\n * ---------------------------------------------------------------------\r\n * \r\n * http://opensource.org/licenses/BSD-2-Clause\r\n * \r\n * Copyright (c) 2009-2015, Emily Stark, Mike Hamburg and Dan Boneh at\r\n * Stanford University. All rights reserved.\r\n * \r\n * Redistribution and use in source and binary forms, with or without\r\n * modification, are permitted provided that the following conditions are\r\n * met:\r\n * \r\n * 1. Redistributions of source code must retain the above copyright\r\n * notice, this list of conditions and the following disclaimer.\r\n * \r\n * 2. Redistributions in binary form must reproduce the above copyright\r\n * notice, this list of conditions and the following disclaimer in the\r\n * documentation and/or other materials provided with the distribution.\r\n * \r\n * THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS \"AS\r\n * IS\" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED\r\n * TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A\r\n * PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT\r\n * HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,\r\n * SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED\r\n * TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR\r\n * PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF\r\n * LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING\r\n * NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS\r\n * SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.\r\n * \r\n * ---------------------------------------------------------------------\r\n * \r\n * http://opensource.org/licenses/GPL-2.0\r\n * \r\n * The Stanford Javascript Crypto Library (hosted here on GitHub) is a\r\n * project by the Stanford Computer Security Lab to build a secure,\r\n * powerful, fast, small, easy-to-use, cross-browser library for\r\n * cryptography in Javascript.\r\n * \r\n * Copyright (c) 2009-2015, Emily Stark, Mike Hamburg and Dan Boneh at\r\n * Stanford University.\r\n * \r\n * This program is free software; you can redistribute it and/or modify it\r\n * under the terms of the GNU General Public License as published by the\r\n * Free Software Foundation; either version 2 of the License, or (at your\r\n * option) any later version.\r\n * \r\n * This program is distributed in the hope that it will be useful, but\r\n * WITHOUT ANY WARRANTY; without even the implied warranty of\r\n * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General\r\n * Public License for more details.\r\n * \r\n * You should have received a copy of the GNU General Public License along\r\n * with this program; if not, write to the Free Software Foundation, Inc.,\r\n * 59 Temple Place, Suite 330, Boston, MA 02111-1307 USA\r\n*/\r\n\r\nvar sjcl={cipher:{},hash:{},keyexchange:{},mode:{},misc:{},codec:{},exception:{corrupt:function(a){this.toString=function(){return\"CORRUPT: \"+this.message};this.message=a},invalid:function(a){this.toString=function(){return\"INVALID: \"+this.message};this.message=a},bug:function(a){this.toString=function(){return\"BUG: \"+this.message};this.message=a},notReady:function(a){this.toString=function(){return\"NOT READY: \"+this.message};this.message=a}}};\r\nsjcl.cipher.aes=function(a){this.s[0][0][0]||this.O();var b,c,d,e,f=this.s[0][4],g=this.s[1];b=a.length;var h=1;if(4!==b&&6!==b&&8!==b)throw new sjcl.exception.invalid(\"invalid aes key size\");this.b=[d=a.slice(0),e=[]];for(a=b;a<4*b+28;a++){c=d[a-1];if(0===a%b||8===b&&4===a%b)c=f[c>>>24]<<24^f[c>>16&255]<<16^f[c>>8&255]<<8^f[c&255],0===a%b&&(c=c<<8^c>>>24^h<<24,h=h<<1^283*(h>>7));d[a]=d[a-b]^c}for(b=0;a;b++,a--)c=d[b&3?a:a-4],e[b]=4>=a||4>b?c:g[0][f[c>>>24]]^g[1][f[c>>16&255]]^g[2][f[c>>8&255]]^g[3][f[c&\r\n255]]};\r\nsjcl.cipher.aes.prototype={encrypt:function(a){return t(this,a,0)},decrypt:function(a){return t(this,a,1)},s:[[[],[],[],[],[]],[[],[],[],[],[]]],O:function(){var a=this.s[0],b=this.s[1],c=a[4],d=b[4],e,f,g,h=[],k=[],l,n,m,p;for(e=0;0x100>e;e++)k[(h[e]=e<<1^283*(e>>7))^e]=e;for(f=g=0;!c[f];f^=l||1,g=k[g]||1)for(m=g^g<<1^g<<2^g<<3^g<<4,m=m>>8^m&255^99,c[f]=m,d[m]=f,n=h[e=h[l=h[f]]],p=0x1010101*n^0x10001*e^0x101*l^0x1010100*f,n=0x101*h[m]^0x1010100*m,e=0;4>e;e++)a[e][f]=n=n<<24^n>>>8,b[e][m]=p=p<<24^p>>>8;for(e=\r\n0;5>e;e++)a[e]=a[e].slice(0),b[e]=b[e].slice(0)}};\r\nfunction t(a,b,c){if(4!==b.length)throw new sjcl.exception.invalid(\"invalid aes block size\");var d=a.b[c],e=b[0]^d[0],f=b[c?3:1]^d[1],g=b[2]^d[2];b=b[c?1:3]^d[3];var h,k,l,n=d.length/4-2,m,p=4,r=[0,0,0,0];h=a.s[c];a=h[0];var q=h[1],v=h[2],w=h[3],x=h[4];for(m=0;m<n;m++)h=a[e>>>24]^q[f>>16&255]^v[g>>8&255]^w[b&255]^d[p],k=a[f>>>24]^q[g>>16&255]^v[b>>8&255]^w[e&255]^d[p+1],l=a[g>>>24]^q[b>>16&255]^v[e>>8&255]^w[f&255]^d[p+2],b=a[b>>>24]^q[e>>16&255]^v[f>>8&255]^w[g&255]^d[p+3],p+=4,e=h,f=k,g=l;for(m=\r\n0;4>m;m++)r[c?3&-m:m]=x[e>>>24]<<24^x[f>>16&255]<<16^x[g>>8&255]<<8^x[b&255]^d[p++],h=e,e=f,f=g,g=b,b=h;return r}\r\nsjcl.bitArray={bitSlice:function(a,b,c){a=sjcl.bitArray.$(a.slice(b/32),32-(b&31)).slice(1);return void 0===c?a:sjcl.bitArray.clamp(a,c-b)},extract:function(a,b,c){var d=Math.floor(-b-c&31);return((b+c-1^b)&-32?a[b/32|0]<<32-d^a[b/32+1|0]>>>d:a[b/32|0]>>>d)&(1<<c)-1},concat:function(a,b){if(0===a.length||0===b.length)return a.concat(b);var c=a[a.length-1],d=sjcl.bitArray.getPartial(c);return 32===d?a.concat(b):sjcl.bitArray.$(b,d,c|0,a.slice(0,a.length-1))},bitLength:function(a){var b=a.length;return 0===\r\nb?0:32*(b-1)+sjcl.bitArray.getPartial(a[b-1])},clamp:function(a,b){if(32*a.length<b)return a;a=a.slice(0,Math.ceil(b/32));var c=a.length;b=b&31;0<c&&b&&(a[c-1]=sjcl.bitArray.partial(b,a[c-1]&2147483648>>b-1,1));return a},partial:function(a,b,c){return 32===a?b:(c?b|0:b<<32-a)+0x10000000000*a},getPartial:function(a){return Math.round(a/0x10000000000)||32},equal:function(a,b){if(sjcl.bitArray.bitLength(a)!==sjcl.bitArray.bitLength(b))return!1;var c=0,d;for(d=0;d<a.length;d++)c|=a[d]^b[d];return 0===\r\nc},$:function(a,b,c,d){var e;e=0;for(void 0===d&&(d=[]);32<=b;b-=32)d.push(c),c=0;if(0===b)return d.concat(a);for(e=0;e<a.length;e++)d.push(c|a[e]>>>b),c=a[e]<<32-b;e=a.length?a[a.length-1]:0;a=sjcl.bitArray.getPartial(e);d.push(sjcl.bitArray.partial(b+a&31,32<b+a?c:d.pop(),1));return d},i:function(a,b){return[a[0]^b[0],a[1]^b[1],a[2]^b[2],a[3]^b[3]]},byteswapM:function(a){var b,c;for(b=0;b<a.length;++b)c=a[b],a[b]=c>>>24|c>>>8&0xff00|(c&0xff00)<<8|c<<24;return a}};\r\nsjcl.codec.utf8String={fromBits:function(a){var b=\"\",c=sjcl.bitArray.bitLength(a),d,e;for(d=0;d<c/8;d++)0===(d&3)&&(e=a[d/4]),b+=String.fromCharCode(e>>>8>>>8>>>8),e<<=8;return decodeURIComponent(escape(b))},toBits:function(a){a=unescape(encodeURIComponent(a));var b=[],c,d=0;for(c=0;c<a.length;c++)d=d<<8|a.charCodeAt(c),3===(c&3)&&(b.push(d),d=0);c&3&&b.push(sjcl.bitArray.partial(8*(c&3),d));return b}};\r\nsjcl.codec.hex={fromBits:function(a){var b=\"\",c;for(c=0;c<a.length;c++)b+=((a[c]|0)+0xf00000000000).toString(16).substr(4);return b.substr(0,sjcl.bitArray.bitLength(a)/4)},toBits:function(a){var b,c=[],d;a=a.replace(/\\s|0x/g,\"\");d=a.length;a=a+\"00000000\";for(b=0;b<a.length;b+=8)c.push(parseInt(a.substr(b,8),16)^0);return sjcl.bitArray.clamp(c,4*d)}};\r\nsjcl.codec.base32={B:\"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567\",X:\"0123456789ABCDEFGHIJKLMNOPQRSTUV\",BITS:32,BASE:5,REMAINING:27,fromBits:function(a,b,c){var d=sjcl.codec.base32.BASE,e=sjcl.codec.base32.REMAINING,f=\"\",g=0,h=sjcl.codec.base32.B,k=0,l=sjcl.bitArray.bitLength(a);c&&(h=sjcl.codec.base32.X);for(c=0;f.length*d<l;)f+=h.charAt((k^a[c]>>>g)>>>e),g<d?(k=a[c]<<d-g,g+=e,c++):(k<<=d,g-=d);for(;f.length&7&&!b;)f+=\"=\";return f},toBits:function(a,b){a=a.replace(/\\s|=/g,\"\").toUpperCase();var c=sjcl.codec.base32.BITS,\r\nd=sjcl.codec.base32.BASE,e=sjcl.codec.base32.REMAINING,f=[],g,h=0,k=sjcl.codec.base32.B,l=0,n,m=\"base32\";b&&(k=sjcl.codec.base32.X,m=\"base32hex\");for(g=0;g<a.length;g++){n=k.indexOf(a.charAt(g));if(0>n){if(!b)try{return sjcl.codec.base32hex.toBits(a)}catch(p){}throw new sjcl.exception.invalid(\"this isn't \"+m+\"!\");}h>e?(h-=e,f.push(l^n>>>h),l=n<<c-h):(h+=d,l^=n<<c-h)}h&56&&f.push(sjcl.bitArray.partial(h&56,l,1));return f}};\r\nsjcl.codec.base32hex={fromBits:function(a,b){return sjcl.codec.base32.fromBits(a,b,1)},toBits:function(a){return sjcl.codec.base32.toBits(a,1)}};\r\nsjcl.codec.base64={B:\"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/\",fromBits:function(a,b,c){var d=\"\",e=0,f=sjcl.codec.base64.B,g=0,h=sjcl.bitArray.bitLength(a);c&&(f=f.substr(0,62)+\"-_\");for(c=0;6*d.length<h;)d+=f.charAt((g^a[c]>>>e)>>>26),6>e?(g=a[c]<<6-e,e+=26,c++):(g<<=6,e-=6);for(;d.length&3&&!b;)d+=\"=\";return d},toBits:function(a,b){a=a.replace(/\\s|=/g,\"\");var c=[],d,e=0,f=sjcl.codec.base64.B,g=0,h;b&&(f=f.substr(0,62)+\"-_\");for(d=0;d<a.length;d++){h=f.indexOf(a.charAt(d));\r\nif(0>h)throw new sjcl.exception.invalid(\"this isn't base64!\");26<e?(e-=26,c.push(g^h>>>e),g=h<<32-e):(e+=6,g^=h<<32-e)}e&56&&c.push(sjcl.bitArray.partial(e&56,g,1));return c}};sjcl.codec.base64url={fromBits:function(a){return sjcl.codec.base64.fromBits(a,1,1)},toBits:function(a){return sjcl.codec.base64.toBits(a,1)}};sjcl.hash.sha256=function(a){this.b[0]||this.O();a?(this.F=a.F.slice(0),this.A=a.A.slice(0),this.l=a.l):this.reset()};sjcl.hash.sha256.hash=function(a){return(new sjcl.hash.sha256).update(a).finalize()};\r\nsjcl.hash.sha256.prototype={blockSize:512,reset:function(){this.F=this.Y.slice(0);this.A=[];this.l=0;return this},update:function(a){\"string\"===typeof a&&(a=sjcl.codec.utf8String.toBits(a));var b,c=this.A=sjcl.bitArray.concat(this.A,a);b=this.l;a=this.l=b+sjcl.bitArray.bitLength(a);if(0x1fffffffffffff<a)throw new sjcl.exception.invalid(\"Cannot hash more than 2^53 - 1 bits\");if(\"undefined\"!==typeof Uint32Array){var d=new Uint32Array(c),e=0;for(b=512+b-(512+b&0x1ff);b<=a;b+=512)u(this,d.subarray(16*e,\r\n16*(e+1))),e+=1;c.splice(0,16*e)}else for(b=512+b-(512+b&0x1ff);b<=a;b+=512)u(this,c.splice(0,16));return this},finalize:function(){var a,b=this.A,c=this.F,b=sjcl.bitArray.concat(b,[sjcl.bitArray.partial(1,1)]);for(a=b.length+2;a&15;a++)b.push(0);b.push(Math.floor(this.l/0x100000000));for(b.push(this.l|0);b.length;)u(this,b.splice(0,16));this.reset();return c},Y:[],b:[],O:function(){function a(a){return 0x100000000*(a-Math.floor(a))|0}for(var b=0,c=2,d,e;64>b;c++){e=!0;for(d=2;d*d<=c;d++)if(0===c%d){e=\r\n!1;break}e&&(8>b&&(this.Y[b]=a(Math.pow(c,.5))),this.b[b]=a(Math.pow(c,1/3)),b++)}}};\r\nfunction u(a,b){var c,d,e,f=a.F,g=a.b,h=f[0],k=f[1],l=f[2],n=f[3],m=f[4],p=f[5],r=f[6],q=f[7];for(c=0;64>c;c++)16>c?d=b[c]:(d=b[c+1&15],e=b[c+14&15],d=b[c&15]=(d>>>7^d>>>18^d>>>3^d<<25^d<<14)+(e>>>17^e>>>19^e>>>10^e<<15^e<<13)+b[c&15]+b[c+9&15]|0),d=d+q+(m>>>6^m>>>11^m>>>25^m<<26^m<<21^m<<7)+(r^m&(p^r))+g[c],q=r,r=p,p=m,m=n+d|0,n=l,l=k,k=h,h=d+(k&l^n&(k^l))+(k>>>2^k>>>13^k>>>22^k<<30^k<<19^k<<10)|0;f[0]=f[0]+h|0;f[1]=f[1]+k|0;f[2]=f[2]+l|0;f[3]=f[3]+n|0;f[4]=f[4]+m|0;f[5]=f[5]+p|0;f[6]=f[6]+r|0;f[7]=\r\nf[7]+q|0}\r\nsjcl.mode.ccm={name:\"ccm\",G:[],listenProgress:function(a){sjcl.mode.ccm.G.push(a)},unListenProgress:function(a){a=sjcl.mode.ccm.G.indexOf(a);-1<a&&sjcl.mode.ccm.G.splice(a,1)},fa:function(a){var b=sjcl.mode.ccm.G.slice(),c;for(c=0;c<b.length;c+=1)b[c](a)},encrypt:function(a,b,c,d,e){var f,g=b.slice(0),h=sjcl.bitArray,k=h.bitLength(c)/8,l=h.bitLength(g)/8;e=e||64;d=d||[];if(7>k)throw new sjcl.exception.invalid(\"ccm: iv must be at least 7 bytes\");for(f=2;4>f&&l>>>8*f;f++);f<15-k&&(f=15-k);c=h.clamp(c,\r\n8*(15-f));b=sjcl.mode.ccm.V(a,b,c,d,e,f);g=sjcl.mode.ccm.C(a,g,c,b,e,f);return h.concat(g.data,g.tag)},decrypt:function(a,b,c,d,e){e=e||64;d=d||[];var f=sjcl.bitArray,g=f.bitLength(c)/8,h=f.bitLength(b),k=f.clamp(b,h-e),l=f.bitSlice(b,h-e),h=(h-e)/8;if(7>g)throw new sjcl.exception.invalid(\"ccm: iv must be at least 7 bytes\");for(b=2;4>b&&h>>>8*b;b++);b<15-g&&(b=15-g);c=f.clamp(c,8*(15-b));k=sjcl.mode.ccm.C(a,k,c,l,e,b);a=sjcl.mode.ccm.V(a,k.data,c,d,e,b);if(!f.equal(k.tag,a))throw new sjcl.exception.corrupt(\"ccm: tag doesn't match\");\r\nreturn k.data},na:function(a,b,c,d,e,f){var g=[],h=sjcl.bitArray,k=h.i;d=[h.partial(8,(b.length?64:0)|d-2<<2|f-1)];d=h.concat(d,c);d[3]|=e;d=a.encrypt(d);if(b.length)for(c=h.bitLength(b)/8,65279>=c?g=[h.partial(16,c)]:0xffffffff>=c&&(g=h.concat([h.partial(16,65534)],[c])),g=h.concat(g,b),b=0;b<g.length;b+=4)d=a.encrypt(k(d,g.slice(b,b+4).concat([0,0,0])));return d},V:function(a,b,c,d,e,f){var g=sjcl.bitArray,h=g.i;e/=8;if(e%2||4>e||16<e)throw new sjcl.exception.invalid(\"ccm: invalid tag length\");\r\nif(0xffffffff<d.length||0xffffffff<b.length)throw new sjcl.exception.bug(\"ccm: can't deal with 4GiB or more data\");c=sjcl.mode.ccm.na(a,d,c,e,g.bitLength(b)/8,f);for(d=0;d<b.length;d+=4)c=a.encrypt(h(c,b.slice(d,d+4).concat([0,0,0])));return g.clamp(c,8*e)},C:function(a,b,c,d,e,f){var g,h=sjcl.bitArray;g=h.i;var k=b.length,l=h.bitLength(b),n=k/50,m=n;c=h.concat([h.partial(8,f-1)],c).concat([0,0,0]).slice(0,4);d=h.bitSlice(g(d,a.encrypt(c)),0,e);if(!k)return{tag:d,data:[]};for(g=0;g<k;g+=4)g>n&&(sjcl.mode.ccm.fa(g/\r\nk),n+=m),c[3]++,e=a.encrypt(c),b[g]^=e[0],b[g+1]^=e[1],b[g+2]^=e[2],b[g+3]^=e[3];return{tag:d,data:h.clamp(b,l)}}};\r\nsjcl.mode.ocb2={name:\"ocb2\",encrypt:function(a,b,c,d,e,f){if(128!==sjcl.bitArray.bitLength(c))throw new sjcl.exception.invalid(\"ocb iv must be 128 bits\");var g,h=sjcl.mode.ocb2.S,k=sjcl.bitArray,l=k.i,n=[0,0,0,0];c=h(a.encrypt(c));var m,p=[];d=d||[];e=e||64;for(g=0;g+4<b.length;g+=4)m=b.slice(g,g+4),n=l(n,m),p=p.concat(l(c,a.encrypt(l(c,m)))),c=h(c);m=b.slice(g);b=k.bitLength(m);g=a.encrypt(l(c,[0,0,0,b]));m=k.clamp(l(m.concat([0,0,0]),g),b);n=l(n,l(m.concat([0,0,0]),g));n=a.encrypt(l(n,l(c,h(c))));\r\nd.length&&(n=l(n,f?d:sjcl.mode.ocb2.pmac(a,d)));return p.concat(k.concat(m,k.clamp(n,e)))},decrypt:function(a,b,c,d,e,f){if(128!==sjcl.bitArray.bitLength(c))throw new sjcl.exception.invalid(\"ocb iv must be 128 bits\");e=e||64;var g=sjcl.mode.ocb2.S,h=sjcl.bitArray,k=h.i,l=[0,0,0,0],n=g(a.encrypt(c)),m,p,r=sjcl.bitArray.bitLength(b)-e,q=[];d=d||[];for(c=0;c+4<r/32;c+=4)m=k(n,a.decrypt(k(n,b.slice(c,c+4)))),l=k(l,m),q=q.concat(m),n=g(n);p=r-32*c;m=a.encrypt(k(n,[0,0,0,p]));m=k(m,h.clamp(b.slice(c),p).concat([0,\r\n0,0]));l=k(l,m);l=a.encrypt(k(l,k(n,g(n))));d.length&&(l=k(l,f?d:sjcl.mode.ocb2.pmac(a,d)));if(!h.equal(h.clamp(l,e),h.bitSlice(b,r)))throw new sjcl.exception.corrupt(\"ocb: tag doesn't match\");return q.concat(h.clamp(m,p))},pmac:function(a,b){var c,d=sjcl.mode.ocb2.S,e=sjcl.bitArray,f=e.i,g=[0,0,0,0],h=a.encrypt([0,0,0,0]),h=f(h,d(d(h)));for(c=0;c+4<b.length;c+=4)h=d(h),g=f(g,a.encrypt(f(h,b.slice(c,c+4))));c=b.slice(c);128>e.bitLength(c)&&(h=f(h,d(h)),c=e.concat(c,[-2147483648,0,0,0]));g=f(g,c);\r\nreturn a.encrypt(f(d(f(h,d(h))),g))},S:function(a){return[a[0]<<1^a[1]>>>31,a[1]<<1^a[2]>>>31,a[2]<<1^a[3]>>>31,a[3]<<1^135*(a[0]>>>31)]}};\r\nsjcl.mode.gcm={name:\"gcm\",encrypt:function(a,b,c,d,e){var f=b.slice(0);b=sjcl.bitArray;d=d||[];a=sjcl.mode.gcm.C(!0,a,f,d,c,e||128);return b.concat(a.data,a.tag)},decrypt:function(a,b,c,d,e){var f=b.slice(0),g=sjcl.bitArray,h=g.bitLength(f);e=e||128;d=d||[];e<=h?(b=g.bitSlice(f,h-e),f=g.bitSlice(f,0,h-e)):(b=f,f=[]);a=sjcl.mode.gcm.C(!1,a,f,d,c,e);if(!g.equal(a.tag,b))throw new sjcl.exception.corrupt(\"gcm: tag doesn't match\");return a.data},ka:function(a,b){var c,d,e,f,g,h=sjcl.bitArray.i;e=[0,0,\r\n0,0];f=b.slice(0);for(c=0;128>c;c++){(d=0!==(a[Math.floor(c/32)]&1<<31-c%32))&&(e=h(e,f));g=0!==(f[3]&1);for(d=3;0<d;d--)f[d]=f[d]>>>1|(f[d-1]&1)<<31;f[0]>>>=1;g&&(f[0]^=-0x1f000000)}return e},j:function(a,b,c){var d,e=c.length;b=b.slice(0);for(d=0;d<e;d+=4)b[0]^=0xffffffff&c[d],b[1]^=0xffffffff&c[d+1],b[2]^=0xffffffff&c[d+2],b[3]^=0xffffffff&c[d+3],b=sjcl.mode.gcm.ka(b,a);return b},C:function(a,b,c,d,e,f){var g,h,k,l,n,m,p,r,q=sjcl.bitArray;m=c.length;p=q.bitLength(c);r=q.bitLength(d);h=q.bitLength(e);\r\ng=b.encrypt([0,0,0,0]);96===h?(e=e.slice(0),e=q.concat(e,[1])):(e=sjcl.mode.gcm.j(g,[0,0,0,0],e),e=sjcl.mode.gcm.j(g,e,[0,0,Math.floor(h/0x100000000),h&0xffffffff]));h=sjcl.mode.gcm.j(g,[0,0,0,0],d);n=e.slice(0);d=h.slice(0);a||(d=sjcl.mode.gcm.j(g,h,c));for(l=0;l<m;l+=4)n[3]++,k=b.encrypt(n),c[l]^=k[0],c[l+1]^=k[1],c[l+2]^=k[2],c[l+3]^=k[3];c=q.clamp(c,p);a&&(d=sjcl.mode.gcm.j(g,h,c));a=[Math.floor(r/0x100000000),r&0xffffffff,Math.floor(p/0x100000000),p&0xffffffff];d=sjcl.mode.gcm.j(g,d,a);k=b.encrypt(e);\r\nd[0]^=k[0];d[1]^=k[1];d[2]^=k[2];d[3]^=k[3];return{tag:q.bitSlice(d,0,f),data:c}}};sjcl.misc.hmac=function(a,b){this.W=b=b||sjcl.hash.sha256;var c=[[],[]],d,e=b.prototype.blockSize/32;this.w=[new b,new b];a.length>e&&(a=b.hash(a));for(d=0;d<e;d++)c[0][d]=a[d]^909522486,c[1][d]=a[d]^1549556828;this.w[0].update(c[0]);this.w[1].update(c[1]);this.R=new b(this.w[0])};\r\nsjcl.misc.hmac.prototype.encrypt=sjcl.misc.hmac.prototype.mac=function(a){if(this.aa)throw new sjcl.exception.invalid(\"encrypt on already updated hmac called!\");this.update(a);return this.digest(a)};sjcl.misc.hmac.prototype.reset=function(){this.R=new this.W(this.w[0]);this.aa=!1};sjcl.misc.hmac.prototype.update=function(a){this.aa=!0;this.R.update(a)};sjcl.misc.hmac.prototype.digest=function(){var a=this.R.finalize(),a=(new this.W(this.w[1])).update(a).finalize();this.reset();return a};\r\nsjcl.misc.pbkdf2=function(a,b,c,d,e){c=c||1E4;if(0>d||0>c)throw new sjcl.exception.invalid(\"invalid params to pbkdf2\");\"string\"===typeof a&&(a=sjcl.codec.utf8String.toBits(a));\"string\"===typeof b&&(b=sjcl.codec.utf8String.toBits(b));e=e||sjcl.misc.hmac;a=new e(a);var f,g,h,k,l=[],n=sjcl.bitArray;for(k=1;32*l.length<(d||1);k++){e=f=a.encrypt(n.concat(b,[k]));for(g=1;g<c;g++)for(f=a.encrypt(f),h=0;h<f.length;h++)e[h]^=f[h];l=l.concat(e)}d&&(l=n.clamp(l,d));return l};\r\nsjcl.prng=function(a){this.c=[new sjcl.hash.sha256];this.m=[0];this.P=0;this.H={};this.N=0;this.U={};this.Z=this.f=this.o=this.ha=0;this.b=[0,0,0,0,0,0,0,0];this.h=[0,0,0,0];this.L=void 0;this.M=a;this.D=!1;this.K={progress:{},seeded:{}};this.u=this.ga=0;this.I=1;this.J=2;this.ca=0x10000;this.T=[0,48,64,96,128,192,0x100,384,512,768,1024];this.da=3E4;this.ba=80};\r\nsjcl.prng.prototype={randomWords:function(a,b){var c=[],d;d=this.isReady(b);var e;if(d===this.u)throw new sjcl.exception.notReady(\"generator isn't seeded\");if(d&this.J){d=!(d&this.I);e=[];var f=0,g;this.Z=e[0]=(new Date).valueOf()+this.da;for(g=0;16>g;g++)e.push(0x100000000*Math.random()|0);for(g=0;g<this.c.length&&(e=e.concat(this.c[g].finalize()),f+=this.m[g],this.m[g]=0,d||!(this.P&1<<g));g++);this.P>=1<<this.c.length&&(this.c.push(new sjcl.hash.sha256),this.m.push(0));this.f-=f;f>this.o&&(this.o=\r\nf);this.P++;this.b=sjcl.hash.sha256.hash(this.b.concat(e));this.L=new sjcl.cipher.aes(this.b);for(d=0;4>d&&(this.h[d]=this.h[d]+1|0,!this.h[d]);d++);}for(d=0;d<a;d+=4)0===(d+1)%this.ca&&y(this),e=z(this),c.push(e[0],e[1],e[2],e[3]);y(this);return c.slice(0,a)},setDefaultParanoia:function(a,b){if(0===a&&\"Setting paranoia=0 will ruin your security; use it only for testing\"!==b)throw new sjcl.exception.invalid(\"Setting paranoia=0 will ruin your security; use it only for testing\");this.M=a},addEntropy:function(a,\r\nb,c){c=c||\"user\";var d,e,f=(new Date).valueOf(),g=this.H[c],h=this.isReady(),k=0;d=this.U[c];void 0===d&&(d=this.U[c]=this.ha++);void 0===g&&(g=this.H[c]=0);this.H[c]=(this.H[c]+1)%this.c.length;switch(typeof a){case \"number\":void 0===b&&(b=1);this.c[g].update([d,this.N++,1,b,f,1,a|0]);break;case \"object\":c=Object.prototype.toString.call(a);if(\"[object Uint32Array]\"===c){e=[];for(c=0;c<a.length;c++)e.push(a[c]);a=e}else for(\"[object Array]\"!==c&&(k=1),c=0;c<a.length&&!k;c++)\"number\"!==typeof a[c]&&\r\n(k=1);if(!k){if(void 0===b)for(c=b=0;c<a.length;c++)for(e=a[c];0<e;)b++,e=e>>>1;this.c[g].update([d,this.N++,2,b,f,a.length].concat(a))}break;case \"string\":void 0===b&&(b=a.length);this.c[g].update([d,this.N++,3,b,f,a.length]);this.c[g].update(a);break;default:k=1}if(k)throw new sjcl.exception.bug(\"random: addEntropy only supports number, array of numbers or string\");this.m[g]+=b;this.f+=b;h===this.u&&(this.isReady()!==this.u&&A(\"seeded\",Math.max(this.o,this.f)),A(\"progress\",this.getProgress()))},\r\nisReady:function(a){a=this.T[void 0!==a?a:this.M];return this.o&&this.o>=a?this.m[0]>this.ba&&(new Date).valueOf()>this.Z?this.J|this.I:this.I:this.f>=a?this.J|this.u:this.u},getProgress:function(a){a=this.T[a?a:this.M];return this.o>=a?1:this.f>a?1:this.f/a},startCollectors:function(){if(!this.D){this.a={loadTimeCollector:B(this,this.ma),mouseCollector:B(this,this.oa),keyboardCollector:B(this,this.la),accelerometerCollector:B(this,this.ea),touchCollector:B(this,this.qa)};if(window.addEventListener)window.addEventListener(\"load\",\r\nthis.a.loadTimeCollector,!1),window.addEventListener(\"mousemove\",this.a.mouseCollector,!1),window.addEventListener(\"keypress\",this.a.keyboardCollector,!1),window.addEventListener(\"devicemotion\",this.a.accelerometerCollector,!1),window.addEventListener(\"touchmove\",this.a.touchCollector,!1);else if(document.attachEvent)document.attachEvent(\"onload\",this.a.loadTimeCollector),document.attachEvent(\"onmousemove\",this.a.mouseCollector),document.attachEvent(\"keypress\",this.a.keyboardCollector);else throw new sjcl.exception.bug(\"can't attach event\");\r\nthis.D=!0}},stopCollectors:function(){this.D&&(window.removeEventListener?(window.removeEventListener(\"load\",this.a.loadTimeCollector,!1),window.removeEventListener(\"mousemove\",this.a.mouseCollector,!1),window.removeEventListener(\"keypress\",this.a.keyboardCollector,!1),window.removeEventListener(\"devicemotion\",this.a.accelerometerCollector,!1),window.removeEventListener(\"touchmove\",this.a.touchCollector,!1)):document.detachEvent&&(document.detachEvent(\"onload\",this.a.loadTimeCollector),document.detachEvent(\"onmousemove\",\r\nthis.a.mouseCollector),document.detachEvent(\"keypress\",this.a.keyboardCollector)),this.D=!1)},addEventListener:function(a,b){this.K[a][this.ga++]=b},removeEventListener:function(a,b){var c,d,e=this.K[a],f=[];for(d in e)e.hasOwnProperty(d)&&e[d]===b&&f.push(d);for(c=0;c<f.length;c++)d=f[c],delete e[d]},la:function(){C(this,1)},oa:function(a){var b,c;try{b=a.x||a.clientX||a.offsetX||0,c=a.y||a.clientY||a.offsetY||0}catch(d){c=b=0}0!=b&&0!=c&&this.addEntropy([b,c],2,\"mouse\");C(this,0)},qa:function(a){a=\r\na.touches[0]||a.changedTouches[0];this.addEntropy([a.pageX||a.clientX,a.pageY||a.clientY],1,\"touch\");C(this,0)},ma:function(){C(this,2)},ea:function(a){a=a.accelerationIncludingGravity.x||a.accelerationIncludingGravity.y||a.accelerationIncludingGravity.z;if(window.orientation){var b=window.orientation;\"number\"===typeof b&&this.addEntropy(b,1,\"accelerometer\")}a&&this.addEntropy(a,2,\"accelerometer\");C(this,0)}};\r\nfunction A(a,b){var c,d=sjcl.random.K[a],e=[];for(c in d)d.hasOwnProperty(c)&&e.push(d[c]);for(c=0;c<e.length;c++)e[c](b)}function C(a,b){\"undefined\"!==typeof window&&window.performance&&\"function\"===typeof window.performance.now?a.addEntropy(window.performance.now(),b,\"loadtime\"):a.addEntropy((new Date).valueOf(),b,\"loadtime\")}function y(a){a.b=z(a).concat(z(a));a.L=new sjcl.cipher.aes(a.b)}function z(a){for(var b=0;4>b&&(a.h[b]=a.h[b]+1|0,!a.h[b]);b++);return a.L.encrypt(a.h)}\r\nfunction B(a,b){return function(){b.apply(a,arguments)}}sjcl.random=new sjcl.prng(6);\r\na:try{var D,E,F,G;if(G=\"undefined\"!==typeof module&&module.exports){var H;try{H=require(\"crypto\")}catch(a){H=null}G=E=H}if(G&&E.randomBytes)D=E.randomBytes(128),D=new Uint32Array((new Uint8Array(D)).buffer),sjcl.random.addEntropy(D,1024,\"crypto['randomBytes']\");else if(\"undefined\"!==typeof window&&\"undefined\"!==typeof Uint32Array){F=new Uint32Array(32);if(window.crypto&&window.crypto.getRandomValues)window.crypto.getRandomValues(F);else if(window.msCrypto&&window.msCrypto.getRandomValues)window.msCrypto.getRandomValues(F);\r\nelse break a;sjcl.random.addEntropy(F,1024,\"crypto['getRandomValues']\")}}catch(a){\"undefined\"!==typeof window&&window.console&&(console.log(\"There was an error collecting entropy from the browser:\"),console.log(a))}\r\nsjcl.json={defaults:{v:1,iter:1E4,ks:128,ts:64,mode:\"ccm\",adata:\"\",cipher:\"aes\"},ja:function(a,b,c,d){c=c||{};d=d||{};var e=sjcl.json,f=e.g({iv:sjcl.random.randomWords(4,0)},e.defaults),g;e.g(f,c);c=f.adata;\"string\"===typeof f.salt&&(f.salt=sjcl.codec.base64.toBits(f.salt));\"string\"===typeof f.iv&&(f.iv=sjcl.codec.base64.toBits(f.iv));if(!sjcl.mode[f.mode]||!sjcl.cipher[f.cipher]||\"string\"===typeof a&&100>=f.iter||64!==f.ts&&96!==f.ts&&128!==f.ts||128!==f.ks&&192!==f.ks&&0x100!==f.ks||2>f.iv.length||\r\n4<f.iv.length)throw new sjcl.exception.invalid(\"json encrypt: invalid parameters\");\"string\"===typeof a?(g=sjcl.misc.cachedPbkdf2(a,f),a=g.key.slice(0,f.ks/32),f.salt=g.salt):sjcl.ecc&&a instanceof sjcl.ecc.elGamal.publicKey&&(g=a.kem(),f.kemtag=g.tag,a=g.key.slice(0,f.ks/32));\"string\"===typeof b&&(b=sjcl.codec.utf8String.toBits(b));\"string\"===typeof c&&(f.adata=c=sjcl.codec.utf8String.toBits(c));g=new sjcl.cipher[f.cipher](a);e.g(d,f);d.key=a;f.ct=\"ccm\"===f.mode&&sjcl.arrayBuffer&&sjcl.arrayBuffer.ccm&&\r\nb instanceof ArrayBuffer?sjcl.arrayBuffer.ccm.encrypt(g,b,f.iv,c,f.ts):sjcl.mode[f.mode].encrypt(g,b,f.iv,c,f.ts);return f},encrypt:function(a,b,c,d){var e=sjcl.json,f=e.ja.apply(e,arguments);return e.encode(f)},ia:function(a,b,c,d){c=c||{};d=d||{};var e=sjcl.json;b=e.g(e.g(e.g({},e.defaults),b),c,!0);var f,g;f=b.adata;\"string\"===typeof b.salt&&(b.salt=sjcl.codec.base64.toBits(b.salt));\"string\"===typeof b.iv&&(b.iv=sjcl.codec.base64.toBits(b.iv));if(!sjcl.mode[b.mode]||!sjcl.cipher[b.cipher]||\"string\"===\r\ntypeof a&&100>=b.iter||64!==b.ts&&96!==b.ts&&128!==b.ts||128!==b.ks&&192!==b.ks&&0x100!==b.ks||!b.iv||2>b.iv.length||4<b.iv.length)throw new sjcl.exception.invalid(\"json decrypt: invalid parameters\");\"string\"===typeof a?(g=sjcl.misc.cachedPbkdf2(a,b),a=g.key.slice(0,b.ks/32),b.salt=g.salt):sjcl.ecc&&a instanceof sjcl.ecc.elGamal.secretKey&&(a=a.unkem(sjcl.codec.base64.toBits(b.kemtag)).slice(0,b.ks/32));\"string\"===typeof f&&(f=sjcl.codec.utf8String.toBits(f));g=new sjcl.cipher[b.cipher](a);f=\"ccm\"===\r\nb.mode&&sjcl.arrayBuffer&&sjcl.arrayBuffer.ccm&&b.ct instanceof ArrayBuffer?sjcl.arrayBuffer.ccm.decrypt(g,b.ct,b.iv,b.tag,f,b.ts):sjcl.mode[b.mode].decrypt(g,b.ct,b.iv,f,b.ts);e.g(d,b);d.key=a;return 1===c.raw?f:sjcl.codec.utf8String.fromBits(f)},decrypt:function(a,b,c,d){var e=sjcl.json;return e.ia(a,e.decode(b),c,d)},encode:function(a){var b,c=\"{\",d=\"\";for(b in a)if(a.hasOwnProperty(b)){if(!b.match(/^[a-z0-9]+$/i))throw new sjcl.exception.invalid(\"json encode: invalid property name\");c+=d+'\"'+\r\nb+'\":';d=\",\";switch(typeof a[b]){case \"number\":case \"boolean\":c+=a[b];break;case \"string\":c+='\"'+escape(a[b])+'\"';break;case \"object\":c+='\"'+sjcl.codec.base64.fromBits(a[b],0)+'\"';break;default:throw new sjcl.exception.bug(\"json encode: unsupported type\");}}return c+\"}\"},decode:function(a){a=a.replace(/\\s/g,\"\");if(!a.match(/^\\{.*\\}$/))throw new sjcl.exception.invalid(\"json decode: this isn't json!\");a=a.replace(/^\\{|\\}$/g,\"\").split(/,/);var b={},c,d;for(c=0;c<a.length;c++){if(!(d=a[c].match(/^\\s*(?:([\"']?)([a-z][a-z0-9]*)\\1)\\s*:\\s*(?:(-?\\d+)|\"([a-z0-9+\\/%*_.@=\\-]*)\"|(true|false))$/i)))throw new sjcl.exception.invalid(\"json decode: this isn't json!\");\r\nnull!=d[3]?b[d[2]]=parseInt(d[3],10):null!=d[4]?b[d[2]]=d[2].match(/^(ct|adata|salt|iv)$/)?sjcl.codec.base64.toBits(d[4]):unescape(d[4]):null!=d[5]&&(b[d[2]]=\"true\"===d[5])}return b},g:function(a,b,c){void 0===a&&(a={});if(void 0===b)return a;for(var d in b)if(b.hasOwnProperty(d)){if(c&&void 0!==a[d]&&a[d]!==b[d])throw new sjcl.exception.invalid(\"required parameter overridden\");a[d]=b[d]}return a},sa:function(a,b){var c={},d;for(d in a)a.hasOwnProperty(d)&&a[d]!==b[d]&&(c[d]=a[d]);return c},ra:function(a,\r\nb){var c={},d;for(d=0;d<b.length;d++)void 0!==a[b[d]]&&(c[b[d]]=a[b[d]]);return c}};sjcl.encrypt=sjcl.json.encrypt;sjcl.decrypt=sjcl.json.decrypt;sjcl.misc.pa={};sjcl.misc.cachedPbkdf2=function(a,b){var c=sjcl.misc.pa,d;b=b||{};d=b.iter||1E3;c=c[a]=c[a]||{};d=c[d]=c[d]||{firstSalt:b.salt&&b.salt.length?b.salt.slice(0):sjcl.random.randomWords(2,0)};c=void 0===b.salt?d.firstSalt:b.salt;d[c]=d[c]||sjcl.misc.pbkdf2(a,c,b.iter);return{key:d[c].slice(0),salt:c.slice(0)}};\r\n\"undefined\"!==typeof module&&module.exports&&(module.exports=sjcl);\"function\"===typeof define&&define([],function(){return sjcl});\r\n","./SeaMonster/stanford-crypto-sha256.js":"/*\r\n * Copyright (C) 2018 Apple Inc. All rights reserved.\r\n *\r\n * Redistribution and use in source and binary forms, with or without\r\n * modification, are permitted provided that the following conditions\r\n * are met:\r\n * 1. Redistributions of source code must retain the above copyright\r\n *    notice, this list of conditions and the following disclaimer.\r\n * 2. Redistributions in binary form must reproduce the above copyright\r\n *    notice, this list of conditions and the following disclaimer in the\r\n *    documentation and/or other materials provided with the distribution.\r\n *\r\n * THIS SOFTWARE IS PROVIDED BY APPLE INC. AND ITS CONTRIBUTORS ``AS IS''\r\n * AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO,\r\n * THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR\r\n * PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL APPLE INC. OR ITS CONTRIBUTORS\r\n * BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR\r\n * CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF\r\n * SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS\r\n * INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN\r\n * CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)\r\n * ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF\r\n * THE POSSIBILITY OF SUCH DAMAGE.\r\n*/\r\n\r\nclass Benchmark {\r\n    runIteration() {\r\n        let hash = sjcl.hash.sha256.hash(\"b4d\")\r\n        let start = Date.now();\r\n        for (let i = 0; i < 4500; ++i) {\r\n            hash = sjcl.hash.sha256.hash(hash);\r\n        }\r\n        if (sjcl.codec.hex.fromBits(hash) !== \"719043495be84b97fe4f5d7e61c99d6d1ba0cd6974a6b10c684c25a44ddd0c03\")\r\n            throw new Error(\"Bad result\");\r\n    }\r\n}\r\n"};
var readFile = function (name) {
    const normalized = String(name).replaceAll("\\", "/");
    if (!Object.prototype.hasOwnProperty.call(__jetstreamResources, normalized))
        throw new Error("JetStream resource not embedded: " + normalized);
    return __jetstreamResources[normalized];
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
            return `load("${url}");`

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
        for (const script of this.scripts)
            globalObject.loadString(script);

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


function initializeJetStreamBenchmark(target, plan) {
    target.plan = plan;
    target.iterations = testIterationCount || plan.iterations || defaultIterationCount;
    target.isAsync = !!plan.isAsync;
    target.scripts = plan.files.map((file) => readFile(file));
    target._resourcesPromise = Promise.resolve();
}

class JetStreamBenchmarkBase {
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
        if (JetStreamParams.testWorstCaseCount)
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

class DefaultBenchmark {
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

class AsyncBenchmark {
    constructor(plan) {
        initializeJetStreamBenchmark(this, plan);
        this.worstCaseCount = plan.worstCaseCount || defaultWorstCaseCount;
        this.firstIteration = null;
        this.worst4 = null;
        this.average = null;
    }
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

class WSLBenchmark {
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
    .catch((error) => print("JetStream2 failed:", error && error.stack ? error.stack : error));
undefined;
