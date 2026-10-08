import type { ControlRecord, TeamDecision, WorkspaceAction } from '@waku/client';
import { router, useLocalSearchParams } from 'expo-router';
import { useEffect, useRef, useState } from 'react';
import { ActivityIndicator, ScrollView, StyleSheet, Text, View } from 'react-native';
import { AutomationButton, AutomationRefresh } from '@/components/automation-controls';
import { ConnectionBanner } from '@/components/connection-banner';
import { useScreenHeaderInset } from '@/components/screen-header';
import { useAutomationQuery } from '@/hooks/use-automation-query';
import { useTaskState } from '@/hooks/use-daemon-data';
import { useTheme } from '@/hooks/use-theme';

type Source = { type: 'decision'; decision: TeamDecision } | { type: 'action'; action: WorkspaceAction; revision: number };
export default function InboxSourceScreen() {
  const params = useLocalSearchParams<{ source: string; id: string; run: string; owner: string }>();
  const theme=useTheme(); const inset=useScreenHeaderInset(); const tasks=useTaskState();
  const [busy,setBusy]=useState(false); const [message,setMessage]=useState<string|null>(null);
  const state=useAutomationQuery(['inbox-source',params.source,params.id,params.run,params.owner],async(client):Promise<Source>=> {
    if (!params.id || (params.source!=='teamDecision' && params.source!=='workspaceAction')) throw new Error('This notification has no reviewable source.');
    if(params.source==='teamDecision') {
      if (!params.run) throw new Error('The team run is unavailable.');
      const response=await client.request({type:'getTeamRun',runId:params.run});
      if(response.type!=='teamRun') throw new Error('Unexpected team response.');
      const decision=response.snapshot.decisions.find((d)=>d.id===params.id);
      if(!decision) throw new Error('This decision is no longer available. Its notification remains in history.');
      return {type:'decision',decision};
    }
    const response=await client.request({type:'loadWorkspaceControl'});
    if(response.type!=='workspaceControl') throw new Error('Unexpected workspace response.');
    const action=response.snapshot.records.find((r):r is Extract<ControlRecord,{kind:'action'}>=>r.kind==='action' && r.id===params.id);
    if(!action) throw new Error('This proposal is no longer available. Its notification remains in history.');
    return {type:'action',action,revision:response.snapshot.revision};
  });
  const {daemon,data,query}=state;
  const scope=`${daemon.activeProfile?.id}:${daemon.activeProfile?.address}:${daemon.connections}`;
  const scopeRef=useRef(scope);scopeRef.current=scope;const mounted=useRef(true); const generation=useRef(0);const locked=useRef(false);const visible=useRef(state.visible);visible.current=state.visible;
  useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;};},[]);
  useEffect(()=>{generation.current++;locked.current=false;setBusy(false);setMessage(null);},[scope]);
  const correctOwner=params.owner===daemon.activeProfile?.id;
  const fresh=state.fresh && correctOwner;
  const pending=data?.type==='decision'?data.decision.status==='pending':data?.action.status==='pending';
  async function resolve(approve:boolean) {
    const client=daemon.client;if(!fresh || !client || !data || !pending || locked.current)return;
    const ticket=++generation.current;const started=scope;locked.current=true;setBusy(true);setMessage(null);
    try {
      if(data.type==='decision') {
        const response=await client.request({type:'resolveTeamDecision',decisionId:data.decision.id,expectedRevision:data.decision.revision,approve});
        if(response.type!=='teamDecision')throw new Error('The decision was not confirmed. Refresh before retrying.');
      } else {
        if(approve && !tasks.data?.projects.some((p)=>p.id===data.action.projectId))throw new Error('The proposal’s project is unavailable. Refresh or restore the project before accepting.');
        const response=await client.request({type:'saveWorkspaceRecord',expectedRevision:data.revision,record:{...data.action,kind:'action',status:approve?'accepted':'declined'}});
        if(response.type!=='workspaceControl')throw new Error('The proposal update was not confirmed. Refresh before retrying.');
      }
    } catch(cause) {if(mounted.current && generation.current===ticket && scopeRef.current===started)setMessage(cause instanceof Error?cause.message:String(cause));}
    finally {if(generation.current===ticket){locked.current=false;if(mounted.current && scopeRef.current===started){setBusy(false);if(visible.current)await query.refetch();}}}
  }
  function prepare() {if(!fresh || busy || data?.type!=='action' || data.action.status!=='accepted' || data.action.actionKind!=='proposedTask')return;router.push({pathname:'/new-task',params:{proposal:data.action.id,owner:daemon.activeProfile!.id}});}
  const title=data?.type==='decision'?data.decision.title:data?.action.title;
  const detail=data?.type==='decision'?data.decision.proposal:data?.action.detail;
  const status=data?.type==='decision'?data.decision.status:data?.action.status;
  return <ScrollView style={{flex:1,backgroundColor:theme.background}} contentContainerStyle={[styles.content,{paddingTop:inset+12}]}>
    <ConnectionBanner/><AutomationRefresh refreshing={query.isFetching} onRefresh={()=>void query.refetch()}/>
    {!correctOwner && <Text accessibilityRole="alert" style={[styles.body,{color:theme.danger}]}>Reconnect to the notification’s owner to review it.</Text>}
    {(message || query.error) && <Text accessibilityRole="alert" style={[styles.body,{color:theme.danger}]}>{message ?? query.error?.message}</Text>}
    {query.isFetching && !data && <ActivityIndicator color={theme.accent}/>}
    {data && <><Text style={[styles.title,{color:theme.text}]}>{title}</Text><Text style={[styles.body,{color:theme.textSecondary}]}>{status}{!fresh?' · Saved state':''}</Text><Text selectable style={[styles.body,{color:theme.text}]}>{detail}</Text>
      <View style={styles.actions}>{pending && <><AutomationButton label={data.type==='decision'?'Approve':'Accept'} disabled={!fresh || busy} onPress={()=>void resolve(true)}/><AutomationButton label="Decline" destructive disabled={!fresh || busy} onPress={()=>void resolve(false)}/></>}
      {data.type==='action' && data.action.actionKind==='proposedTask' && data.action.status==='accepted' && <AutomationButton label="Prepare New task" disabled={!fresh || busy} onPress={prepare}/>}</View>
      {data.type==='action' && data.action.actionKind==='proposedTask' && <Text style={[styles.body,{color:theme.textSecondary}]}>Acceptance prepares work for review. Send from New task when ready.</Text>}
    </>}
  </ScrollView>;
}
const styles=StyleSheet.create({content:{paddingHorizontal:20,paddingBottom:32,gap:16},title:{fontSize:22,lineHeight:30,fontWeight:'600'},body:{fontSize:15,lineHeight:23},actions:{flexDirection:'row',flexWrap:'wrap',gap:8}});
