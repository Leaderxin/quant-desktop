// 日志桥必须在应用模块图之前安装。
//
// 静态 import 会先把整张依赖图求值完，入口模块体才执行 —— 如果等到 main.ts
// 的函数体里再装，App.vue 及其依赖的求值期错误已经错过了桥。所以这个模块只
// 做一件事，并由两个入口放在**所有其它 import 之前**引入。
import { installWebLogging, windowSource } from './utils/webLog';

installWebLogging(windowSource(window.location.pathname));
