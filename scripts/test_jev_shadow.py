"""Offline pilot tests, including concurrency, negative paths and fresh-clone recovery."""

import copy
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import threading
import unittest
from unittest import mock

import jev_client
import jev_shadow as pilot

ROOT = Path(__file__).resolve().parents[1]


def fake_result(payload, choice=None):
    criteria = payload['questions']['judgment']['criteria']
    selected = choice or next(iter(criteria))
    return {'status': 'ok', 'elapsed_seconds': 0.01, 'http_status': 200,
            'response': {'model': payload['model'], 'usage': {'input_tokens': 100, 'output_tokens': 20},
                         'answers': {'judgment': {'type': 'choice', 'choice': selected, 'confidence': 1.0,
                            'probabilities': {key: float(key == selected) for key in criteria}}}}}


class PilotTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        (ROOT/'tmp').mkdir(exist_ok=True)
        cls.directory = tempfile.TemporaryDirectory(prefix='jev-tests-', dir=ROOT/'tmp')
        cls.root = Path(cls.directory.name)
        cls.repo = cls.root/'repo'
        cls.repo.mkdir()
        subprocess.run(['git','init','-q',str(cls.repo)],check=True)
        (cls.repo/'.gitignore').write_text('secret/\ntmp/\n')
        (cls.repo/'evidence.txt').write_text('The script has not been executed.\nActual Rust assertion failed.\n')
        (cls.repo/'scripts').mkdir()
        for name in ('jev_shadow.py','jev_client.py'):
            shutil.copyfile(ROOT/'scripts'/name,cls.repo/'scripts'/name)
        shutil.copytree(ROOT/'docs/process/jev-shadow',cls.repo/'docs/process/jev-shadow')
        shutil.copyfile(ROOT/'docs/process/JEV_SHADOW_PILOT.md',cls.repo/'docs/process/JEV_SHADOW_PILOT.md')
        cls.commit(cls.repo)

    @staticmethod
    def commit(repo):
        pilot.git(repo,'add','.')
        pilot.git(repo,'-c','user.name=Pilot Test','-c','user.email=pilot@example.invalid','commit','-qm','fixture')

    @classmethod
    def tearDownClass(cls):
        cls.directory.cleanup()

    def setUp(self):
        self.local = tempfile.TemporaryDirectory(dir=self.root)
        self.store = Path(self.local.name)/'store'
        self.cfg = pilot.config()
        self.db = pilot.connect(self.store,self.cfg)
        self.env = mock.patch.dict(os.environ, {'TYPESAFE_API_KEY':'synthetic-unit-test-key'})
        self.env.start()
        self.packet = {
            'id':'case-one', 'task_id':'task-one', 'kind':'claims', 'sample_kind':'development',
            'input':{'evidence':'The script has not been executed.', 'claim':'The script executed successfully.'},
            'baseline':{'choice':'supports','actor':'author-session'},
            'source_refs':['evidence.txt'], 'execution':None,
        }

    def tearDown(self):
        self.db.close()
        self.env.stop()
        self.local.cleanup()

    def record(self, line='v0.2.3', packet=None):
        return pilot.record(self.db,self.repo,line,packet or self.packet,self.cfg)

    def evaluate(self, choice='contradicts'):
        row = pilot.sample(self.db,self.packet['id'],'v0.2.3')
        return pilot.run_evaluation(self.db,row['id'],'v0.2.3',row['request_hash'],self.cfg,
                                    client=lambda payload,key:fake_result(payload,choice))

    def label(self, independent=True):
        label = {'choice':'contradicts','reviewer':'reviewer-session','independent':independent,'rationale':'Explicit nonexecution.'}
        return pilot.label_case(self.db,self.packet['id'],'v0.2.3',label,self.cfg)

    def finish(self):
        return pilot.finish(self.db,'v0.2.3','task-one',{'resolved':True,'system2_input_tokens':None,
            'system2_output_tokens':None,'elapsed_seconds':1.0,'notes':'Observed test fixture outcome.'})

    def test_capture_has_provenance_and_request_excludes_baseline(self):
        self.record()
        preview = pilot.preview(self.db,'case-one','v0.2.3')
        self.assertEqual(preview['outbound_request']['state'],{'input':self.packet['input']})
        self.assertNotIn('author-session',json.dumps(preview))
        row = pilot.sample(self.db,'case-one','v0.2.3')
        self.assertEqual(json.loads(row['document'])['snapshot']['head'],pilot.git(self.repo,'rev-parse','HEAD').decode().strip())
        self.assertEqual(self.record()['status'],'recorded')
        with self.assertRaises(ValueError):
            self.record('v0.3.0')
        self.packet['baseline']['choice'] = 'unsupported'
        with self.assertRaises(ValueError):
            self.record()

    def test_no_command_execution_and_literal_quote_required(self):
        self.packet['input']['evidence'] = 'A source statement that does not exist.'
        with self.assertRaises(ValueError):
            self.record()
        self.assertEqual(self.db.execute('SELECT COUNT(*) FROM samples').fetchone()[0],0)

    def test_private_paths_traversal_secrets_and_oversize_rejected(self):
        for ref in ('../evidence.txt','/etc/passwd','secret/notes.txt','.env','.codex/config.toml'):
            packet=copy.deepcopy(self.packet)
            packet['source_refs']=[ref]
            with self.subTest(ref=ref), self.assertRaises(ValueError):
                self.record(packet=packet)
        for value in ('synthetic-unit-test-key','Authorization: Bearer fake','x'*13000):
            packet=copy.deepcopy(self.packet)
            packet['input']['claim']=value
            with self.subTest(value=value[:20]), self.assertRaises(ValueError):
                self.record(packet=packet)

    def test_symlink_source_escape_rejected(self):
        outside=self.root/'outside.txt'
        outside.write_text(self.packet['input']['evidence'])
        link=self.repo/'escape.txt'
        link.symlink_to(outside)
        try:
            self.packet['source_refs']=['escape.txt']
            with self.assertRaises(ValueError):
                self.record()
        finally:
            link.unlink()
            outside.unlink()

    def test_request_review_digest_and_duplicate_evaluation(self):
        row=self.record()
        client=mock.Mock(side_effect=lambda payload,key:fake_result(payload,'contradicts'))
        with self.assertRaises(ValueError):
            pilot.run_evaluation(self.db,'case-one','v0.2.3','wrong',self.cfg,client)
        for _ in range(2):
            result=pilot.run_evaluation(self.db,'case-one','v0.2.3',row['request_sha256'],self.cfg,client)
            self.assertTrue(result['answers_hidden'])
            self.assertNotIn('response',result)
        self.assertEqual(client.call_count,1)
        self.assertEqual(self.db.execute('SELECT reserved FROM samples').fetchone()[0],100)

    def test_blinding_and_independent_comparison(self):
        self.record()
        self.evaluate()
        blind=pilot.blind_case(self.db,'case-one','v0.2.3')
        self.assertNotIn('baseline',blind)
        self.assertNotIn('evaluation',blind)
        with self.assertRaises(ValueError):
            pilot.reveal(self.db,'case-one','v0.2.3')
        self.label()
        group=pilot.report(self.db,self.cfg)['groups']['v0.2.3/claims']
        self.assertEqual(group['independently_labelled_and_finished'],0)
        self.finish()
        self.assertEqual(pilot.reveal(self.db,'case-one','v0.2.3')['authority'],'advisory_only')
        report=pilot.report(self.db,self.cfg)
        group=report['groups']['v0.2.3/claims']
        self.assertEqual((group['baseline_correct'],group['jev_correct'],group['corrected']),(0,1,1))
        self.assertEqual(report['tasks']['system2_input_tokens_known_tasks'],0)
        self.assertEqual(report['subscription_savings'],'not_measured_no_control_condition')

    def test_self_labels_and_unfinished_tasks_do_not_inflate_accuracy(self):
        self.record()
        self.evaluate()
        label={'choice':'contradicts','reviewer':'author-session','independent':True,'rationale':'Self review.'}
        with self.assertRaises(ValueError):
            pilot.label_case(self.db,'case-one','v0.2.3',label,self.cfg)
        self.label(False)
        self.finish()
        self.assertEqual(pilot.report(self.db,self.cfg)['groups']['v0.2.3/claims']['independently_labelled_and_finished'],0)
        with self.assertRaises(ValueError):
            self.label(True)

    def test_false_support_and_regression_counted(self):
        self.packet['baseline']['choice']='contradicts'
        self.record()
        self.evaluate('supports')
        self.label()
        self.finish()
        group=pilot.report(self.db,self.cfg)['groups']['v0.2.3/claims']
        self.assertEqual((group['false_supports'],group['regressed']),(1,1))

    def test_synthetic_excluded_from_default_report(self):
        self.packet['sample_kind']='synthetic'
        self.record()
        self.evaluate()
        self.assertFalse(pilot.report(self.db,self.cfg)['groups'])
        self.assertEqual(pilot.report(self.db,self.cfg,True)['groups']['v0.2.3/claims']['ok'],1)

    def test_new_baselines_cannot_follow_task_completion_or_mix_cohorts(self):
        self.record()
        packet=copy.deepcopy(self.packet)
        packet['id']='new-case'
        packet['sample_kind']='synthetic'
        with self.assertRaises(ValueError):
            self.record(packet=packet)
        packet['sample_kind']='development'
        self.finish()
        with self.assertRaises(ValueError):
            self.record(packet=packet)

    def test_missing_credentials_and_service_failure_are_unavailable(self):
        row=self.record()
        client=mock.Mock()
        with mock.patch.dict(os.environ,{'TYPESAFE_API_KEY':''}):
            result=pilot.run_evaluation(self.db,'case-one','v0.2.3',row['request_sha256'],self.cfg,client)
        self.assertEqual(result['status'],'unavailable')
        client.assert_not_called()
        self.assertEqual(pilot.report(self.db,self.cfg)['groups']['v0.2.3/claims']['unavailable'],1)
        self.packet['id']='second-case'
        row=self.record()
        unavailable=lambda payload,key:{'status':'unavailable','reason':'service_error','http_status':403,'elapsed_seconds':.1}
        pilot.run_evaluation(self.db,'second-case','v0.2.3',row['request_sha256'],self.cfg,unavailable)
        self.assertEqual(pilot.sample(self.db,'second-case','v0.2.3')['reserved'],64000)

    def test_budget_reservation_prevents_overspend(self):
        row=self.record()
        limits=dict(self.cfg,max_input_tokens=63999)
        client=mock.Mock()
        result=pilot.run_evaluation(self.db,'case-one','v0.2.3',row['request_sha256'],limits,client)
        self.assertEqual(result['status'],'budget_exhausted')
        client.assert_not_called()

    def test_configuration_change_requires_separate_store(self):
        with self.assertRaises(ValueError):
            pilot.connect(self.store,dict(self.cfg,max_calls=1))
        self.assertEqual(self.db.execute('SELECT COUNT(*) FROM samples').fetchone()[0],0)

    def test_late_response_does_not_overwrite_manual_interruption(self):
        row=self.record()
        def late(payload,key):
            pilot.recover(self.db,'case-one','v0.2.3')
            return fake_result(payload,'contradicts')
        result=pilot.run_evaluation(self.db,'case-one','v0.2.3',row['request_sha256'],self.cfg,late)
        self.assertEqual(result['status'],'unavailable')
        self.assertEqual(pilot.sample(self.db,'case-one','v0.2.3')['reserved'],64000)

    def test_concurrent_duplicate_and_global_budget(self):
        first=self.record()
        packet=copy.deepcopy(self.packet)
        packet['id']='case-two'
        second=self.record('v0.3.0',packet)
        entered,release=threading.Event(),threading.Event()
        failures=[]
        def slow(payload,key):
            entered.set()
            if not release.wait(5):
                raise RuntimeError('test synchronization failed')
            return fake_result(payload,'contradicts')
        def worker():
            db=pilot.connect(self.store,self.cfg)
            try:
                pilot.run_evaluation(db,'case-one','v0.2.3',first['request_sha256'],self.cfg,slow)
            except BaseException as error:
                failures.append(type(error).__name__)
            finally:
                db.close()
        thread=threading.Thread(target=worker)
        thread.start()
        try:
            self.assertTrue(entered.wait(5))
            client=mock.Mock()
            duplicate=pilot.run_evaluation(self.db,'case-one','v0.2.3',first['request_sha256'],self.cfg,client)
            self.assertEqual(duplicate['status'],'in_flight_or_interrupted')
            full=pilot.run_evaluation(self.db,'case-two','v0.3.0',second['request_sha256'],dict(self.cfg,max_input_tokens=64000),client)
            self.assertEqual(full['status'],'budget_exhausted')
            client.assert_not_called()
        finally:
            release.set()
            thread.join(5)
        self.assertFalse(thread.is_alive())
        self.assertEqual(failures,[])

    def test_crash_recovery_retains_reservation(self):
        row=self.record()
        with self.assertRaises(KeyboardInterrupt):
            pilot.run_evaluation(self.db,'case-one','v0.2.3',row['request_sha256'],self.cfg,mock.Mock(side_effect=KeyboardInterrupt))
        self.assertEqual(pilot.recover(self.db,'case-one','v0.2.3')['status'],'unavailable')
        self.assertEqual(pilot.sample(self.db,'case-one','v0.2.3')['reserved'],64000)
        with self.assertRaises(ValueError):
            pilot.recover(self.db,'case-one','v0.2.3')

    def test_restore_rejects_tamper_and_is_transactional(self):
        self.record()
        self.evaluate()
        checkpoint=pilot.export_data(self.db)
        checkpoint['samples'][0]['request_hash']='bad'
        target=pilot.connect(Path(self.local.name)/'target',self.cfg)
        try:
            with self.assertRaises(ValueError):
                pilot.restore(target,checkpoint,self.cfg)
            self.assertEqual(target.execute('SELECT COUNT(*) FROM tasks').fetchone()[0],0)
            self.assertEqual(target.execute('SELECT COUNT(*) FROM samples').fetchone()[0],0)
        finally:
            target.close()

    def test_restore_inflight_never_retries(self):
        row=self.record()
        with self.assertRaises(KeyboardInterrupt):
            pilot.run_evaluation(self.db,'case-one','v0.2.3',row['request_sha256'],self.cfg,mock.Mock(side_effect=KeyboardInterrupt))
        target=pilot.connect(Path(self.local.name)/'target',self.cfg)
        try:
            pilot.restore(target,pilot.export_data(self.db),self.cfg)
            restored=pilot.sample(target,'case-one','v0.2.3')
            self.assertEqual(json.loads(restored['evaluation'])['reason'],'restored_inflight_billing_unknown')
            self.assertEqual(restored['reserved'],64000)
        finally:
            target.close()

    def test_fresh_git_clone_restores_without_old_private_trees(self):
        self.record()
        self.evaluate()
        self.label()
        self.finish()
        source=Path(self.local.name)/'export-source'
        clone=Path(self.local.name)/'fresh-clone'
        subprocess.run(['git','clone','-q','--no-hardlinks',str(self.repo),str(source)],check=True)
        (source/'docs/process/jev-shadow/checkpoint.json').write_text(json.dumps(pilot.export_data(self.db)))
        self.commit(source)
        subprocess.run(['git','clone','-q','--no-hardlinks',str(source),str(clone)],check=True)
        self.assertFalse((clone/'secret').exists())
        self.assertFalse((clone/'tmp').exists())
        command=['python3','-B',str(clone/'scripts/jev_shadow.py'),'--repo',str(clone),'--line','v0.2.3']
        environment=dict(os.environ)
        environment.pop('TYPESAFE_API_KEY',None)
        subprocess.run(command+['install'],check=True,capture_output=True,env=environment)
        completed=subprocess.run(command+['report'],check=True,capture_output=True,text=True,env=environment)
        report=json.loads(completed.stdout)
        self.assertEqual(report['groups']['v0.2.3/claims']['corrected'],1)
        subprocess.run(command+['install'],check=True,capture_output=True,env=environment)


class ClientTests(unittest.TestCase):
    def setUp(self):
        packet=json.loads((ROOT/'docs/process/jev-shadow/examples/claims.json').read_text())
        self.payload=pilot.request_for(packet,pilot.config())
        self.response=fake_result(self.payload,'contradicts')['response']

    def test_credentials_and_excerpt_not_on_argv_and_no_redirects(self):
        key='synthetic-client-key'
        process=mock.Mock(returncode=0,stdout=json.dumps(self.response)+'\nHTTP_STATUS=200\n',stderr='')
        with mock.patch.object(jev_client.subprocess,'run',return_value=process) as run:
            result=jev_client.evaluate(self.payload,key)
        self.assertEqual(result['status'],'ok')
        args,kwargs=run.call_args
        self.assertNotIn(key,' '.join(args[0]))
        self.assertNotIn(self.payload['state']['input']['evidence'],' '.join(args[0]))
        self.assertNotIn('--location',args[0])
        self.assertNotIn('TYPESAFE_API_KEY',kwargs['env'])
        self.assertIn(key,kwargs['input'])
        self.assertEqual(kwargs['timeout'],35)

    def test_malformed_probabilities_wrong_model_and_nonfinite_rejected(self):
        for field,value in [('model','other'),('confidence',True),('confidence',float('nan')),('confidence',1.1),('choice','unknown')]:
            response=copy.deepcopy(self.response)
            if field=='model':
                response[field]=value
            else:
                response['answers']['judgment'][field]=value
            with self.subTest(field=field,value=value),self.assertRaises(ValueError):
                jev_client.validate(self.payload,response)

    def test_unrecognized_server_text_is_not_retained(self):
        self.response['debug']='potentially sensitive server text'
        self.response['answers']['judgment']['explanation']='also untrusted'
        validated=jev_client.validate(self.payload,self.response)
        self.assertNotIn('debug',validated)
        self.assertNotIn('explanation',validated['answers']['judgment'])

    def test_service_error_timeout_and_bad_body_do_not_leak(self):
        key='synthetic-client-key'
        for process in (mock.Mock(returncode=0,stdout=key+'\nHTTP_STATUS=403\n',stderr=key),
                        mock.Mock(returncode=0,stdout='not-json\nHTTP_STATUS=200\n',stderr=key)):
            with mock.patch.object(jev_client.subprocess,'run',return_value=process):
                result=jev_client.evaluate(self.payload,key)
            self.assertEqual(result['status'],'unavailable')
            self.assertNotIn(key,json.dumps(result))
            self.assertNotIn('response',result)
        with mock.patch.object(jev_client.subprocess,'run',side_effect=subprocess.TimeoutExpired('curl',35)):
            self.assertEqual(jev_client.evaluate(self.payload,key)['reason'],'transport_error')


if __name__=='__main__':
    unittest.main()
