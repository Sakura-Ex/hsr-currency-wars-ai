"""Rejection checks prevent partial leaves from implying a full modifier import."""
import copy
import unittest
from tools.raw_config.actions import Unsupported
from tools.raw_config.weakness import weakness_attachments


class WeaknessTests(unittest.TestCase):
    def setUp(self):
        self.task = {'$type':'RPG.GameCore.StackWeakness','OPType':'Attach','WeakList':['Wind'],
                     'TargetType':{'$type':'RPG.GameCore.TargetAlias','Alias':'ModifierOwnerEntity'}}

    def test_selected_leaf_keeps_source_offset_and_scope(self):
        result = weakness_attachments([self.task],'modifier/OnStack',start_index=2)
        self.assertEqual(result['elements'],['wind'])
        self.assertEqual(result['scope'],'selected_stack_weakness_leaves_only')
        self.assertEqual(result['operations'][0]['pointer'],'modifier/OnStack/2')

    def test_unimplemented_modifier_work_cannot_disappear(self):
        for key,value in [('$type','RPG.GameCore.StackProperty'),('OPType','Replace'),
                          ('TargetType',{'$type':'RPG.GameCore.TargetAlias','Alias':'AllEnemy'}),
                          ('UnknownNumeric',1)]:
            task=copy.deepcopy(self.task); task[key]=value
            with self.subTest(key=key), self.assertRaises(Unsupported):
                weakness_attachments([task],'modifier')
        task=copy.deepcopy(self.task); del task['OPType']
        with self.assertRaises(Unsupported): weakness_attachments([task],'modifier')

    def test_empty_unknown_and_duplicate_lists_reject(self):
        for values in [[],['Unknown'],['Wind','Wind'],[None]]:
            task=copy.deepcopy(self.task); task['WeakList']=values
            with self.subTest(values=values), self.assertRaises(Unsupported):
                weakness_attachments([task],'modifier')
        with self.assertRaises(Unsupported): weakness_attachments([self.task,self.task],'modifier')
        with self.assertRaises(Unsupported): weakness_attachments([],'modifier')


if __name__=='__main__': unittest.main()
