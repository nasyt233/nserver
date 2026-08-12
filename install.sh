#!/bin/bash
[[ -n $PREFIX ]] && bin=$PREFIX/bin || bin=/usr/bin
dow_url="nasyt.hoha.top/shell/nserver.sh"
echo "正在下载文件";curl --progress-bar -o nserver -L $dow_url;if [[ $? -ne 0 ]]; then echo "文件下载失败,错误代码 $?";exit 1;fi
chmod +x nserver;mv nserver $bin;echo "nserver安装完成";echo "输入nserver进入工具"