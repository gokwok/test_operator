import type { Want } from '@kit.AbilityKit';
import { FormExtensionAbility, formBindingData, formProvider } from '@kit.FormKit';
import { hilog } from '@kit.PerformanceAnalysisKit';

const HILOG_DOMAIN: number = 0x0000;
const HILOG_TAG: string = 'OperationFormAbility';

interface OperationWidgetFormData {
  title: string;
  shortcut1: string;
  shortcut2: string;
  shortcut3: string;
  shortcut4: string;
}

export default class OperationFormAbility extends FormExtensionAbility {
  onAddForm(want: Want): formBindingData.FormBindingData {
    hilog.info(HILOG_DOMAIN, HILOG_TAG, 'onAddForm want=%{public}s', JSON.stringify(want));
    return formBindingData.createFormBindingData(this.buildDefaultData());
  }

  onUpdateForm(formId: string): void {
    hilog.info(HILOG_DOMAIN, HILOG_TAG, 'onUpdateForm formId=%{public}s', formId);
    const data = this.buildDefaultData();
    formProvider.updateForm(formId, formBindingData.createFormBindingData(data));
  }

  onRemoveForm(formId: string): void {
    hilog.info(HILOG_DOMAIN, HILOG_TAG, 'onRemoveForm formId=%{public}s', formId);
  }

  onAcquireFormState(_want: Want): formProvider.FormState {
    return formProvider.FormState.READY;
  }

  private buildDefaultData(): OperationWidgetFormData {
    const data: OperationWidgetFormData = {
      title: '操作智能体',
      shortcut1: '快捷 1',
      shortcut2: '快捷 2',
      shortcut3: '快捷 3',
      shortcut4: '快捷 4',
    };
    return data;
  }
}

