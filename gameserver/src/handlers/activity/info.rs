use crate::{
    error::AppError,
    net::{context::ConnectionContext, packet::ClientPacket},
};
use prost::Message;
use sonettobuf::{
    CmdId, Get123InfosRequest, Get153InfosRequest, Get158InfosRequest, Get166InfosRequest,
    Get217InfosRequest, GetAct172InfoRequest, GetAct209InfoRequest, GetAct225InfoRequest,
};

pub(super) fn default_activity_id_for_type(type_id: i32) -> Option<i32> {
    config::configs::get().latest_open_activity_id(type_id)
}

macro_rules! info_handler {
    (
        $handler:ident,
        $request:ident,
        $reply:ident,
        $cmd:ident,
        $type_id:literal
    ) => {
        pub async fn $handler(
            ctx: &mut ConnectionContext,
            req: ClientPacket,
        ) -> Result<(), AppError> {
            let msg = sonettobuf::$request::decode(&req.data[..])?;
            let mut reply = sonettobuf::$reply::default();
            reply.activity_id = msg
                .activity_id
                .or_else(|| default_activity_id_for_type($type_id));
            ctx.send_reply(CmdId::$cmd, reply, 0, req.up_tag).await
        }
    };
    ($handler:ident, $request:ident, $reply:ident, $cmd:ident) => {
        pub async fn $handler(
            ctx: &mut ConnectionContext,
            req: ClientPacket,
        ) -> Result<(), AppError> {
            sonettobuf::$request::decode(&req.data[..])?;
            ctx.send_reply(CmdId::$cmd, sonettobuf::$reply::default(), 0, req.up_tag)
                .await
        }
    };
}

pub async fn on_act1000_get_info(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    ctx.player()?;
    let msg = sonettobuf::Act1000GetInfoRequest::decode(&req.data[..])?;
    let reply = sonettobuf::Act1000GetInfoReply {
        activity_id: msg
            .activity_id
            .or_else(|| default_activity_id_for_type(1000)),
        account_bind_bonus: None,
    };

    ctx.send_reply(CmdId::Act1000GetInfoCmd, reply, 0, req.up_tag)
        .await
}

info_handler!(
    on_act1001_get_info,
    Act1001GetInfoRequest,
    Act1001GetInfoReply,
    Act1001GetInfoCmd,
    1001
);
info_handler!(
    on_get_106_infos,
    Get106InfosRequest,
    Get106InfosReply,
    Get106InfosCmd,
    106
);
info_handler!(
    on_get_108_infos,
    Get108InfosRequest,
    Get108InfosReply,
    Get108InfosCmd,
    108
);
info_handler!(
    on_get_act109_info,
    GetAct109InfoRequest,
    GetAct109InfoReply,
    GetAct109InfoCmd,
    109
);
info_handler!(
    on_get_111_infos,
    Get111InfosRequest,
    Get111InfosReply,
    Get111InfosCmd,
    111
);
info_handler!(
    on_get_112_infos,
    Get112InfosRequest,
    Get112InfosReply,
    Get112InfosCmd,
    112
);
info_handler!(
    on_get_act113_info,
    GetAct113InfoRequest,
    GetAct113InfoReply,
    GetAct113InfoCmd,
    113
);
info_handler!(
    on_get_114_infos,
    Get114InfosRequest,
    Get114InfosReply,
    Get114InfosCmd,
    114
);
info_handler!(
    on_get_act115_info,
    GetAct115InfoRequest,
    GetAct115InfoReply,
    GetAct115InfoCmd,
    115
);
info_handler!(
    on_get_116_infos,
    Get116InfosRequest,
    Get116InfosReply,
    Get116InfosCmd,
    116
);
info_handler!(
    on_get_act120_info,
    GetAct120InfoRequest,
    GetAct120InfoReply,
    GetAct120InfoCmd,
    120
);
info_handler!(
    on_get_121_infos,
    Get121InfosRequest,
    Get121InfosReply,
    Get121InfosCmd
);
info_handler!(
    on_get_act122_infos,
    GetAct122InfosRequest,
    GetAct122InfosReply,
    GetAct122InfosCmd,
    122
);
info_handler!(
    on_get_act124_infos,
    GetAct124InfosRequest,
    GetAct124InfosReply,
    GetAct124InfosCmd,
    124
);
info_handler!(
    on_get_126_infos,
    Get126InfosRequest,
    Get126InfosReply,
    Get126InfosCmd,
    126
);
pub async fn on_get_128_infos(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = sonettobuf::Get128InfosRequest::decode(&req.data[..])?;
    let reply = ctx
        .player()?
        .activity
        .act128_info(ctx.state.db, msg.activity_id)
        .await?;
    ctx.send_reply(CmdId::Get128InfosCmd, reply, 0, req.up_tag)
        .await
}
info_handler!(
    on_get_129_infos,
    Get129InfosRequest,
    Get129InfosReply,
    Get129InfosCmd,
    129
);
info_handler!(
    on_get_130_infos,
    Get130InfosRequest,
    Get130InfosReply,
    Get130InfosCmd,
    130
);
info_handler!(
    on_get_131_infos,
    Get131InfosRequest,
    Get131InfosReply,
    Get131InfosCmd,
    131
);
info_handler!(
    on_get_132_infos,
    Get132InfosRequest,
    Get132InfosReply,
    Get132InfosCmd,
    132
);
info_handler!(
    on_get_133_infos,
    Get133InfosRequest,
    Get133InfosReply,
    Get133InfosCmd,
    133
);
info_handler!(
    on_get_134_infos,
    Get134InfosRequest,
    Get134InfosReply,
    Get134InfosCmd,
    134
);
info_handler!(
    on_get_139_infos,
    Get139InfosRequest,
    Get139InfosReply,
    Get139InfosCmd,
    139
);
info_handler!(
    on_get_140_infos,
    Get140InfosRequest,
    Get140InfosReply,
    Get140InfosCmd,
    140
);
info_handler!(
    on_get_act142_info,
    GetAct142InfoRequest,
    GetAct142InfoReply,
    GetAct142InfoCmd,
    142
);
info_handler!(
    on_get_144_infos,
    Get144InfosRequest,
    Get144InfosReply,
    Get144InfosCmd
);
info_handler!(
    on_get_145_infos,
    Get145InfosRequest,
    Get145InfosReply,
    Get145InfosCmd,
    145
);
info_handler!(
    on_get_act147_infos,
    GetAct147InfosRequest,
    GetAct147InfosReply,
    GetAct147InfosCmd,
    147
);
info_handler!(
    on_get_148_info,
    Get148InfoRequest,
    Get148InfoReply,
    Get148InfoCmd,
    148
);
info_handler!(
    on_get_149_info,
    Get149InfoRequest,
    Get149InfoReply,
    Get149InfoCmd,
    149
);
info_handler!(
    on_get_157_info,
    Get157InfoRequest,
    Get157InfoReply,
    Get157InfoCmd,
    157
);
info_handler!(
    on_get_159_infos,
    Get159InfosRequest,
    Get159InfosReply,
    Get159InfosCmd,
    159
);
info_handler!(
    on_act161_get_info,
    Act161GetInfoRequest,
    Act161GetInfoReply,
    Act161GetInfoCmd,
    161
);
info_handler!(
    on_get_163_infos,
    Get163InfosRequest,
    Get163InfosReply,
    Get163InfosCmd
);
info_handler!(
    on_get_act164_info,
    GetAct164InfoRequest,
    GetAct164InfoReply,
    GetAct164InfoCmd,
    164
);
info_handler!(
    on_get_act167_info,
    GetAct167InfoRequest,
    GetAct167InfoReply,
    GetAct167InfoCmd,
    167
);
info_handler!(
    on_get_168_infos,
    Get168InfosRequest,
    Get168InfosReply,
    Get168InfosCmd,
    168
);
info_handler!(
    on_get_169_info,
    Get169InfoRequest,
    Get169InfoReply,
    Get169InfoCmd,
    169
);
info_handler!(
    on_get_170_info,
    Get170InfoRequest,
    Get170InfoReply,
    Get170InfoCmd
);
info_handler!(
    on_get_171_info,
    Get171InfoRequest,
    Get171InfoReply,
    Get171InfoCmd,
    171
);
info_handler!(
    on_get_act174_info,
    GetAct174InfoRequest,
    GetAct174InfoReply,
    GetAct174InfoCmd,
    174
);
info_handler!(
    on_get_act178_info,
    GetAct178InfoRequest,
    GetAct178InfoReply,
    GetAct178InfoCmd,
    178
);
info_handler!(
    on_get_179_infos,
    Get179InfosRequest,
    Get179InfosReply,
    Get179InfosCmd,
    179
);
info_handler!(
    on_get_180_infos,
    Get180InfosRequest,
    Get180InfosReply,
    Get180InfosCmd,
    180
);
info_handler!(
    on_get_181_infos,
    Get181InfosRequest,
    Get181InfosReply,
    Get181InfosCmd,
    181
);
info_handler!(
    on_get_act182_info,
    GetAct182InfoRequest,
    GetAct182InfoReply,
    GetAct182InfoCmd,
    182
);
info_handler!(
    on_act183_get_info,
    Act183GetInfoRequest,
    Act183GetInfoReply,
    Act183GetInfoCmd,
    183
);
info_handler!(
    on_get_act184_info,
    GetAct184InfoRequest,
    GetAct184InfoReply,
    GetAct184InfoCmd,
    184
);
info_handler!(
    on_get_act185_info,
    GetAct185InfoRequest,
    GetAct185InfoReply,
    GetAct185InfoCmd,
    185
);
info_handler!(
    on_get_187_info,
    Get187InfoRequest,
    Get187InfoReply,
    Get187InfoCmd,
    187
);
info_handler!(
    on_get_act188_info,
    GetAct188InfoRequest,
    GetAct188InfoReply,
    GetAct188InfoCmd,
    188
);
info_handler!(
    on_get_act190_info,
    GetAct190InfoRequest,
    GetAct190InfoReply,
    GetAct190InfoCmd,
    190
);
info_handler!(
    on_get_act191_info,
    GetAct191InfoRequest,
    GetAct191InfoReply,
    GetAct191InfoCmd,
    191
);
info_handler!(
    on_get_act192_info,
    GetAct192InfoRequest,
    GetAct192InfoReply,
    GetAct192InfoCmd,
    192
);
info_handler!(
    on_act194_get_infos,
    Act194GetInfosRequest,
    Act194GetInfosReply,
    Act194GetInfosCmd,
    194
);
info_handler!(
    on_get_201_info,
    Get201InfoRequest,
    Get201InfoReply,
    Get201InfoCmd,
    201
);
info_handler!(
    on_get_act203_info,
    GetAct203InfoRequest,
    GetAct203InfoReply,
    GetAct203InfoCmd,
    203
);
info_handler!(
    on_get_act204_info,
    GetAct204InfoRequest,
    GetAct204InfoReply,
    GetAct204InfoCmd,
    204
);
info_handler!(
    on_get_act210_info,
    GetAct210InfoRequest,
    GetAct210InfoReply,
    GetAct210InfoCmd,
    210
);
info_handler!(
    on_get_act211_info,
    GetAct211InfoRequest,
    GetAct211InfoReply,
    GetAct211InfoCmd,
    211
);
info_handler!(
    on_get_act215_info,
    GetAct215InfoRequest,
    GetAct215InfoReply,
    GetAct215InfoCmd,
    215
);
info_handler!(
    on_get_act220_info,
    GetAct220InfoRequest,
    GetAct220InfoReply,
    GetAct220InfoCmd,
    220
);
info_handler!(
    on_get_act223_info,
    GetAct223InfoRequest,
    GetAct223InfoReply,
    GetAct223InfoCmd,
    223
);
info_handler!(
    on_get_act224_info,
    GetAct224InfoRequest,
    GetAct224InfoReply,
    GetAct224InfoCmd,
    224
);
info_handler!(
    on_get_act226_info,
    GetAct226InfoRequest,
    GetAct226InfoReply,
    GetAct226InfoCmd,
    226
);
info_handler!(
    on_get_act231_info,
    GetAct231InfoRequest,
    GetAct231InfoReply,
    GetAct231InfoCmd,
    231
);
info_handler!(
    on_act240_get_info,
    Act240GetInfoRequest,
    Act240GetInfoReply,
    Act240GetInfoCmd,
    240
);

pub async fn on_get_act235_info(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = sonettobuf::GetAct235InfoRequest::decode(&req.data[..])?;
    let reply = sonettobuf::GetAct235InfoReply {
        activity_id: msg
            .activity_id
            .or_else(|| default_activity_id_for_type(235)),
        info: Some(sonettobuf::Act235Info {
            total_reward_count: Some(0),
            preparation_ids: Vec::new(),
            count_list: Vec::new(),
        }),
    };

    ctx.send_reply(CmdId::GetAct235InfoCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_get_act172_info(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = GetAct172InfoRequest::decode(&req.data[..])?;
    let db = ctx.state.db;
    let reply = ctx
        .player_mut()?
        .activity
        .act172_info(db, msg.activity_id)
        .await?;

    ctx.send_reply(CmdId::GetAct172InfoCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_get_123_infos(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = Get123InfosRequest::decode(&req.data[..])?;
    let reply = ctx.player_mut()?.activity.act123_infos(msg.activity_id);
    ctx.send_reply(CmdId::Get123InfosCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_get_153_infos(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = Get153InfosRequest::decode(&req.data[..])?;
    let reply = ctx.player_mut()?.activity.act153_infos(msg.activity_id);
    ctx.send_reply(CmdId::Get153InfosCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_get_158_infos(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = Get158InfosRequest::decode(&req.data[..])?;
    let reply = ctx.player()?.activity.act158_infos(msg.activity_id);
    ctx.send_reply(CmdId::Get158InfosCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_get_166_infos(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = Get166InfosRequest::decode(&req.data[..])?;
    let reply = ctx.player()?.activity.act166_infos(msg.activity_id);
    ctx.send_reply(CmdId::Get166InfosCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_get_act209_info(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = GetAct209InfoRequest::decode(&req.data[..])?;
    let db = ctx.state.db;
    let reply = ctx
        .player_mut()?
        .activity
        .act209_info(db, msg.activity_id)
        .await?;
    ctx.send_reply(CmdId::GetAct209InfoCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_get_217_infos(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = Get217InfosRequest::decode(&req.data[..])?;
    let db = ctx.state.db;
    let reply = ctx
        .player_mut()?
        .activity
        .act217_infos(db, msg.activity_id)
        .await?;
    ctx.send_reply(CmdId::Get217InfosCmd, reply, 0, req.up_tag)
        .await
}

pub async fn on_get_act225_info(
    ctx: &mut ConnectionContext,
    req: ClientPacket,
) -> Result<(), AppError> {
    let msg = GetAct225InfoRequest::decode(&req.data[..])?;
    let db = ctx.state.db;
    let reply = ctx
        .player_mut()?
        .activity
        .act225_info(db, msg.activity_id)
        .await?;
    ctx.send_reply(CmdId::GetAct225InfoCmd, reply, 0, req.up_tag)
        .await
}
